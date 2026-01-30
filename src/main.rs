#![windows_subsystem = "windows"]

use slint::ComponentHandle;
use std::sync::{Arc, Mutex, atomic::{AtomicBool, AtomicU32, Ordering}};
use std::thread;
use std::rc::Rc;
use std::cell::RefCell;

mod services;
use services::{
    settings::SettingsService,
    options::GameModeOptions,
    gamemode::GameModeService,
    update::UpdateService,
    revi_tweaks::ReviTweaksService,
    advanced_modules::AdvancedModulesService,
};

slint::include_modules!();

/// Check if a process with the given PID is still running
fn is_process_running(pid: u32) -> bool {
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
    use windows::Win32::Foundation::CloseHandle;
    
    unsafe {
        if let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
            let _ = CloseHandle(handle);
            true
        } else {
            false
        }
    }
}

/// Trim our own working set to minimize memory when idle/hidden
#[inline]
fn trim_own_memory() {
    use windows::Win32::System::ProcessStatus::EmptyWorkingSet;
    use windows::Win32::System::Threading::GetCurrentProcess;
    
    unsafe {
        let _ = EmptyWorkingSet(GetCurrentProcess());
    }
}

fn main() -> Result<(), slint::PlatformError> {
    // === RENDERING OPTIMIZATION ===
    std::env::set_var("SLINT_FONT_HINTING", "none");
    std::env::set_var("SLINT_ENABLE_SUBPIXEL_RENDERING", "1");
    
    let ui = AppWindow::new()?;
    let ui_handle = ui.as_weak();

    // 1. Load Settings
    let settings_service = SettingsService::new();
    let loaded_settings = settings_service.load();
    let app_settings = Arc::new(Mutex::new(loaded_settings.clone()));

    // 2. Initialize UI State from Settings (including advanced_tweaks and disable_mpo)
    let initial_settings_ui = AppSettings {
        suspend_explorer: loaded_settings.suspend_explorer,
        suspend_browsers: loaded_settings.suspend_browsers,
        suspend_launchers: loaded_settings.suspend_launchers,
        advanced_tweaks: loaded_settings.advanced_tweaks,
        disable_mpo: loaded_settings.disable_mpo,
        run_on_startup: loaded_settings.run_on_startup,
    };
    ui.set_settings(initial_settings_ui);
    
    // Initialize Advanced Module Settings
    let initial_advanced_ui = AdvancedSettings {
        disable_core_parking: loaded_settings.advanced_modules.disable_core_parking,
        enable_large_pages: loaded_settings.advanced_modules.enable_large_pages,
        mmcss_priority_boost: loaded_settings.advanced_modules.mmcss_priority_boost,
        enable_hags: loaded_settings.advanced_modules.enable_hags,
        process_idle_demotion: loaded_settings.advanced_modules.process_idle_demotion,
        lower_bufferbloat: loaded_settings.advanced_modules.lower_bufferbloat,
    };
    ui.set_advanced_settings(initial_advanced_ui);
    
    // Initialize bufferbloat status from current system state
    ui.set_bufferbloat_active(AdvancedModulesService::get_bufferbloat_status());
    
    // Create advanced modules service
    let advanced_modules_service = Arc::new(AdvancedModulesService::new());

    // 3. Window Moving Logic
    let ui_handle_copy = ui_handle.clone();
    ui.on_move_window(move |delta_x, delta_y| {
        let _ = ui_handle_copy.upgrade_in_event_loop(move |ui| {
             let window = ui.window();
             let current_pos = window.position();
             let scale = window.scale_factor();
             let dx_phys = delta_x * scale;
             let dy_phys = delta_y * scale;
             window.set_position(slint::PhysicalPosition::new(
                 current_pos.x + dx_phys as i32,
                 current_pos.y + dy_phys as i32
             ));
        });
    });

    // 4. Shared state for game process monitoring and game mode active status
    let monitored_pid: Arc<AtomicU32> = Arc::new(AtomicU32::new(0));
    let is_monitoring: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
    let is_game_mode_active: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
    
    let settings_clone = app_settings.clone();
    let gamemode_service = Arc::new(Mutex::new(GameModeService::new()));
    let gm_clone = gamemode_service.clone();
    let monitored_pid_clone = monitored_pid.clone();
    let is_monitoring_clone = is_monitoring.clone();
    let advanced_modules_clone = advanced_modules_service.clone();

    // 5. Game Process Monitor - Background thread (memory optimized)
    let ui_handle_monitor = ui.as_weak();
    let gamemode_for_monitor = gamemode_service.clone();
    let settings_for_monitor = app_settings.clone();
    let monitored_pid_for_thread = monitored_pid.clone();
    let is_monitoring_for_thread = is_monitoring.clone();
    let advanced_modules_for_monitor = advanced_modules_service.clone();
    let is_active_for_monitor = is_game_mode_active.clone();
    
    thread::spawn(move || {
        loop {
            // Adaptive sleep: 2s when monitoring, 5s when idle to save resources
            let sleep_secs = if is_monitoring_for_thread.load(Ordering::Relaxed) { 2 } else { 5 };
            thread::sleep(std::time::Duration::from_secs(sleep_secs));
            
            if !is_monitoring_for_thread.load(Ordering::Acquire) {
                continue;
            }
            
            let pid = monitored_pid_for_thread.load(Ordering::Acquire);
            if pid == 0 {
                continue;
            }
            
            if !is_process_running(pid) {
                is_monitoring_for_thread.store(false, Ordering::Release);
                monitored_pid_for_thread.store(0, Ordering::Release);
                
                // Extract settings once, avoid repeated clones
                let (options, advanced, advanced_modules) = {
                    let guard = settings_for_monitor.lock().unwrap();
                    (
                        GameModeOptions {
                            suspend_explorer: guard.suspend_explorer,
                            suspend_browsers: guard.suspend_browsers,
                            suspend_launchers: guard.suspend_launchers,
                            isolate_network: guard.isolate_network,
                        },
                        guard.advanced_tweaks,
                        guard.advanced_modules.clone(),
                    )
                };
                
                if let Ok(svc) = gamemode_for_monitor.lock() {
                    svc.disable_game_mode(&options);
                }
                
                // Restore ReviOS tweaks if they were enabled
                if advanced {
                    ReviTweaksService::disable();
                }
                
                // Restore advanced modules
                advanced_modules_for_monitor.disable(&advanced_modules);
                
                // Clear active flag
                is_active_for_monitor.store(false, Ordering::SeqCst);
                
                let ui_weak = ui_handle_monitor.clone();
                let _ = ui_weak.upgrade_in_event_loop(move |ui| {
                    ui.set_active(false);
                    ui.window().show().unwrap();
                    let _ = ui.window().set_minimized(false);
                });
            }
        }
    });

    // 6. Toggle Game Mode (with ReviOS tweaks support and advanced modules)
    let advanced_modules_toggle = advanced_modules_clone.clone();
    let is_active_for_toggle = is_game_mode_active.clone();
    ui.on_toggle_game_mode(move |active| {
        let ui_weak = ui_handle.clone();
        let guard = settings_clone.lock().unwrap();
        let options = GameModeOptions {
            suspend_explorer: guard.suspend_explorer,
            suspend_browsers: guard.suspend_browsers,
            suspend_launchers: guard.suspend_launchers,
            isolate_network: guard.isolate_network,
        };
        let advanced = guard.advanced_tweaks;
        let advanced_modules = guard.advanced_modules.clone();
        drop(guard);
        
        let service = gm_clone.clone();
        let pid_ref = monitored_pid_clone.clone();
        let monitoring_ref = is_monitoring_clone.clone();
        let advanced_svc = advanced_modules_toggle.clone();
        let active_flag = is_active_for_toggle.clone();

        thread::spawn(move || {
            if active {
                // Set active flag immediately
                active_flag.store(true, Ordering::SeqCst);
                
                // Apply ReviOS tweaks FIRST if enabled (saves original state)
                if advanced {
                    ReviTweaksService::enable();
                }
                
                // Apply advanced modules
                advanced_svc.enable(&advanced_modules);
                
                if let Ok(mut svc) = service.lock() {
                    svc.enable_game_mode(&options);
                    if let Some((game_pid, _hwnd)) = svc.detect_game() {
                        pid_ref.store(game_pid, Ordering::SeqCst);
                        monitoring_ref.store(true, Ordering::SeqCst);
                    }
                }
                let _ = ui_weak.upgrade_in_event_loop(move |ui| {
                    ui.set_active(true);
                });
            } else {
                monitoring_ref.store(false, Ordering::SeqCst);
                pid_ref.store(0, Ordering::SeqCst);
                
                if let Ok(svc) = service.lock() {
                    svc.disable_game_mode(&options);
                }
                
                // Restore ReviOS tweaks (restores original state)
                if advanced {
                    ReviTweaksService::disable();
                }
                
                // Restore advanced modules
                advanced_svc.disable(&advanced_modules);
                
                // Clear active flag after cleanup
                active_flag.store(false, Ordering::SeqCst);
                
                let _ = ui_weak.upgrade_in_event_loop(move |ui| {
                    ui.set_active(false);
                    ui.window().show().unwrap();
                    let _ = ui.window().set_minimized(false);
                });
            }
        });
    });

    // 7. Settings Changed (including advanced_tweaks and disable_mpo)
    let settings_clone_2 = app_settings.clone();
    let settings_service_arc = Arc::new(settings_service);
    let ss_clone = settings_service_arc.clone();

    ui.on_settings_changed(move |new_settings| {
        let mut guard = settings_clone_2.lock().unwrap();
        guard.suspend_explorer = new_settings.suspend_explorer;
        guard.suspend_browsers = new_settings.suspend_browsers;
        guard.suspend_launchers = new_settings.suspend_launchers;
        guard.advanced_tweaks = new_settings.advanced_tweaks;
        
        // Handle MPO toggle - apply immediately when changed
        if new_settings.disable_mpo != guard.disable_mpo {
            guard.disable_mpo = new_settings.disable_mpo;
            if new_settings.disable_mpo {
                // Disable MPO
                GameModeService::set_mpo_disabled();
            } else {
                // Enable MPO + OverlayMinFPS=0
                GameModeService::set_mpo_enabled();
            }
        }
        
        if new_settings.run_on_startup != guard.run_on_startup {
             guard.run_on_startup = new_settings.run_on_startup;
             if let Ok(auto) = auto_launch::AutoLaunchBuilder::new()
                .set_app_name("XillyGameMode")
                .set_app_path(&std::env::current_exe().unwrap_or_default().to_string_lossy())
                .build() 
             {
                 if guard.run_on_startup {
                     let _ = auto.enable();
                 } else {
                     let _ = auto.disable();
                 }
             }
        }
        ss_clone.save(&guard);
    });

    // 7b. Advanced Settings Changed
    let settings_clone_3 = app_settings.clone();
    let ss_clone_2 = settings_service_arc.clone();
    
    ui.on_advanced_settings_changed(move |new_advanced| {
        let mut guard = settings_clone_3.lock().unwrap();
        guard.advanced_modules.disable_core_parking = new_advanced.disable_core_parking;
        guard.advanced_modules.enable_large_pages = new_advanced.enable_large_pages;
        guard.advanced_modules.mmcss_priority_boost = new_advanced.mmcss_priority_boost;
        guard.advanced_modules.enable_hags = new_advanced.enable_hags;
        guard.advanced_modules.process_idle_demotion = new_advanced.process_idle_demotion;
        guard.advanced_modules.lower_bufferbloat = new_advanced.lower_bufferbloat;
        ss_clone_2.save(&guard);
    });

    // 7c. Permanent Bufferbloat Toggle (On/Off button)
    let ui_handle_bufferbloat = ui.as_weak();
    ui.on_toggle_bufferbloat_permanent(move || {
        let current_state = AdvancedModulesService::get_bufferbloat_status();
        if current_state {
            // Currently ON, turn it OFF
            AdvancedModulesService::set_bufferbloat_disabled();
        } else {
            // Currently OFF, turn it ON
            AdvancedModulesService::set_bufferbloat_enabled();
        }
        // Update UI state
        let _ = ui_handle_bufferbloat.upgrade_in_event_loop(move |ui| {
            ui.set_bufferbloat_active(!current_state);
        });
    });

    // 8. Updates
    ui.on_check_updates(move || {
        UpdateService::check_for_updates();
    });

    // 9. Export Specs - Comprehensive hardware info
    ui.on_export_specs(move || {
        thread::spawn(move || {
            use std::process::Command;
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            
            // CPU: Name, Cores, Threads
            let cpu_info = Command::new("wmic")
                .args(["cpu", "get", "name,NumberOfCores,NumberOfLogicalProcessors", "/format:list"])
                .creation_flags(CREATE_NO_WINDOW)
                .output()
                .map(|o| {
                    let s = String::from_utf8_lossy(&o.stdout);
                    let mut name = String::new();
                    let mut cores = String::new();
                    let mut threads = String::new();
                    for line in s.lines() {
                        let line = line.trim();
                        if let Some(v) = line.strip_prefix("Name=") {
                            name = v.trim().to_string();
                        } else if let Some(v) = line.strip_prefix("NumberOfCores=") {
                            cores = v.trim().to_string();
                        } else if let Some(v) = line.strip_prefix("NumberOfLogicalProcessors=") {
                            threads = v.trim().to_string();
                        }
                    }
                    if !name.is_empty() {
                        format!("{} ({} cores / {} threads)", name, cores, threads)
                    } else {
                        "Unknown".to_string()
                    }
                })
                .unwrap_or_else(|_| "Unknown".to_string());

            // GPUs: All video controllers (iGPU + dGPU)
            let gpus = Command::new("wmic")
                .args(["path", "win32_VideoController", "get", "name,AdapterRAM", "/format:list"])
                .creation_flags(CREATE_NO_WINDOW)
                .output()
                .map(|o| {
                    let s = String::from_utf8_lossy(&o.stdout);
                    let mut gpu_list: Vec<String> = Vec::new();
                    let mut current_name = String::new();
                    let mut current_vram: u64 = 0;
                    
                    for line in s.lines() {
                        let line = line.trim();
                        if let Some(v) = line.strip_prefix("Name=") {
                            if !current_name.is_empty() {
                                // Save previous GPU
                                if current_vram > 0 {
                                    let vram_gb = current_vram as f64 / 1073741824.0;
                                    gpu_list.push(format!("{} ({:.1} GB)", current_name, vram_gb));
                                } else {
                                    gpu_list.push(current_name.clone());
                                }
                            }
                            current_name = v.trim().to_string();
                            current_vram = 0;
                        } else if let Some(v) = line.strip_prefix("AdapterRAM=") {
                            current_vram = v.trim().parse().unwrap_or(0);
                        }
                    }
                    // Don't forget the last GPU
                    if !current_name.is_empty() {
                        if current_vram > 0 {
                            let vram_gb = current_vram as f64 / 1073741824.0;
                            gpu_list.push(format!("{} ({:.1} GB)", current_name, vram_gb));
                        } else {
                            gpu_list.push(current_name);
                        }
                    }
                    
                    if gpu_list.is_empty() {
                        "Unknown".to_string()
                    } else {
                        gpu_list.join("\n       ")
                    }
                })
                .unwrap_or_else(|_| "Unknown".to_string());

            // RAM: Total capacity and speed
            let ram_info = Command::new("wmic")
                .args(["memorychip", "get", "Capacity,Speed", "/format:list"])
                .creation_flags(CREATE_NO_WINDOW)
                .output()
                .map(|o| {
                    let s = String::from_utf8_lossy(&o.stdout);
                    let mut total_capacity: u64 = 0;
                    let mut speed: u32 = 0;
                    let mut stick_count = 0;
                    
                    for line in s.lines() {
                        let line = line.trim();
                        if let Some(v) = line.strip_prefix("Capacity=") {
                            if let Ok(cap) = v.trim().parse::<u64>() {
                                total_capacity += cap;
                                stick_count += 1;
                            }
                        } else if let Some(v) = line.strip_prefix("Speed=") {
                            if let Ok(spd) = v.trim().parse::<u32>() {
                                if spd > speed { speed = spd; }
                            }
                        }
                    }
                    
                    let gb = total_capacity as f64 / 1073741824.0;
                    if speed > 0 {
                        format!("{:.0} GB ({} sticks @ {} MHz)", gb, stick_count, speed)
                    } else {
                        format!("{:.0} GB ({} sticks)", gb, stick_count)
                    }
                })
                .unwrap_or_else(|_| "Unknown".to_string());

            // OS: Caption + Build
            let os_info = Command::new("wmic")
                .args(["os", "get", "caption,BuildNumber,OSArchitecture", "/format:list"])
                .creation_flags(CREATE_NO_WINDOW)
                .output()
                .map(|o| {
                    let s = String::from_utf8_lossy(&o.stdout);
                    let mut caption = String::new();
                    let mut build = String::new();
                    let mut arch = String::new();
                    
                    for line in s.lines() {
                        let line = line.trim();
                        if let Some(v) = line.strip_prefix("Caption=") {
                            caption = v.trim().to_string();
                        } else if let Some(v) = line.strip_prefix("BuildNumber=") {
                            build = v.trim().to_string();
                        } else if let Some(v) = line.strip_prefix("OSArchitecture=") {
                            arch = v.trim().to_string();
                        }
                    }
                    
                    format!("{} (Build {}) {}", caption, build, arch)
                })
                .unwrap_or_else(|_| "Windows".to_string());

            // Motherboard
            let mobo = Command::new("wmic")
                .args(["baseboard", "get", "Manufacturer,Product", "/format:list"])
                .creation_flags(CREATE_NO_WINDOW)
                .output()
                .map(|o| {
                    let s = String::from_utf8_lossy(&o.stdout);
                    let mut manufacturer = String::new();
                    let mut product = String::new();
                    
                    for line in s.lines() {
                        let line = line.trim();
                        if let Some(v) = line.strip_prefix("Manufacturer=") {
                            manufacturer = v.trim().to_string();
                        } else if let Some(v) = line.strip_prefix("Product=") {
                            product = v.trim().to_string();
                        }
                    }
                    format!("{} {}", manufacturer, product)
                })
                .unwrap_or_else(|_| "Unknown".to_string());

            // Storage drives
            let storage = Command::new("wmic")
                .args(["diskdrive", "get", "Model,Size,MediaType", "/format:list"])
                .creation_flags(CREATE_NO_WINDOW)
                .output()
                .map(|o| {
                    let s = String::from_utf8_lossy(&o.stdout);
                    let mut drives: Vec<String> = Vec::new();
                    let mut current_model = String::new();
                    let mut current_size: u64 = 0;
                    let mut current_type = String::new();
                    
                    for line in s.lines() {
                        let line = line.trim();
                        if let Some(v) = line.strip_prefix("Model=") {
                            if !current_model.is_empty() {
                                let gb = current_size as f64 / 1000000000.0;
                                let type_str = if current_type.contains("SSD") || current_type.contains("Solid") { 
                                    "SSD" 
                                } else if current_type.contains("Fixed") {
                                    "HDD"
                                } else {
                                    ""
                                };
                                drives.push(format!("{} ({:.0} GB) {}", current_model, gb, type_str).trim().to_string());
                            }
                            current_model = v.trim().to_string();
                            current_size = 0;
                            current_type.clear();
                        } else if let Some(v) = line.strip_prefix("Size=") {
                            current_size = v.trim().parse().unwrap_or(0);
                        } else if let Some(v) = line.strip_prefix("MediaType=") {
                            current_type = v.trim().to_string();
                        }
                    }
                    if !current_model.is_empty() {
                        let gb = current_size as f64 / 1000000000.0;
                        let type_str = if current_type.contains("SSD") || current_type.contains("Solid") { 
                            "SSD" 
                        } else if current_type.contains("Fixed") {
                            "HDD"
                        } else {
                            ""
                        };
                        drives.push(format!("{} ({:.0} GB) {}", current_model, gb, type_str).trim().to_string());
                    }
                    
                    if drives.is_empty() {
                        "Unknown".to_string()
                    } else {
                        drives.join("\n           ")
                    }
                })
                .unwrap_or_else(|_| "Unknown".to_string());

            let report = format!(
                "System Specs:\n\
                 CPU:     {}\n\
                 GPU:     {}\n\
                 RAM:     {}\n\
                 Mobo:    {}\n\
                 Storage: {}\n\
                 OS:      {}",
                cpu_info, gpus, ram_info, mobo, storage, os_info
            );
            
            let escaped = report.replace("\"", "`\"").replace("\n", "`n");
            let _ = Command::new("powershell")
                .args(["-Command", &format!("Set-Clipboard -Value \"{}\"", escaped)])
                .creation_flags(CREATE_NO_WINDOW)
                .output();

            use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_OK, MB_ICONINFORMATION};
            use windows::Win32::Foundation::HWND;
            use windows::core::HSTRING;
            unsafe {
                MessageBoxW(HWND::default(), &HSTRING::from("System specs copied to clipboard!"), &HSTRING::from("Specs Copied"), MB_OK | MB_ICONINFORMATION);
            }
        });
    });

    // 10. System Tray - Proper implementation with timer-based event polling
    use tray_icon::{TrayIconBuilder, menu::{Menu, MenuItem}, MouseButton, MouseButtonState};
    
    let tray_menu = Menu::new();
    let show_item = MenuItem::new("Show", true, None);
    let exit_item = MenuItem::new("Exit", true, None);
    let _ = tray_menu.append_items(&[&show_item, &exit_item]);

    let icon = {
        let icon_bytes = include_bytes!("../ui/assets/appicon.png");
        let img = image::load_from_memory(icon_bytes).expect("Failed to load icon");
        let rgba = img.resize(32, 32, image::imageops::FilterType::Lanczos3).to_rgba8();
        let (width, height) = rgba.dimensions();
        tray_icon::Icon::from_rgba(rgba.into_raw(), width, height).expect("Failed to create icon")
    };
    
    // Keep tray icon alive by storing in Rc
    let tray_icon = Rc::new(RefCell::new(Some(
        TrayIconBuilder::new()
            .with_menu(Box::new(tray_menu))
            .with_tooltip("Xilly Game Mode")
            .with_icon(icon)
            .build()
            .unwrap()
    )));

    let menu_channel = tray_icon::menu::MenuEvent::receiver();
    let tray_channel = tray_icon::TrayIconEvent::receiver();

    let show_id = show_item.id().clone();
    let exit_id = exit_item.id().clone();
    let is_active_for_tray = is_game_mode_active.clone();
    
    // Use Slint timer for tray event polling (runs in main event loop)
    let ui_handle_tray = ui.as_weak();
    let tray_timer = slint::Timer::default();
    let tray_icon_keeper = tray_icon.clone();
    tray_timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_millis(100),
        move || {
            // Keep tray icon reference alive
            let _keep = tray_icon_keeper.borrow();
            
            // Process menu events
            while let Ok(event) = menu_channel.try_recv() {
                if event.id == exit_id {
                    // Only allow exit if game mode is NOT active
                    if !is_active_for_tray.load(Ordering::SeqCst) {
                        std::process::exit(0);
                    } else {
                        // Show message that user must deactivate game mode first
                        use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_OK, MB_ICONWARNING};
                        use windows::Win32::Foundation::HWND;
                        use windows::core::HSTRING;
                        unsafe {
                            MessageBoxW(
                                HWND::default(), 
                                &HSTRING::from("Cannot exit while Game Mode is active.\nPlease deactivate Game Mode first."), 
                                &HSTRING::from("Xilly Game Mode"), 
                                MB_OK | MB_ICONWARNING
                            );
                        }
                    }
                } else if event.id == show_id {
                    if let Some(ui) = ui_handle_tray.upgrade() {
                        let _ = ui.window().show();
                        let _ = ui.window().set_minimized(false);
                    }
                }
            }
            
            // Process tray click events
            while let Ok(event) = tray_channel.try_recv() {
                if let tray_icon::TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                    if let Some(ui) = ui_handle_tray.upgrade() {
                        if ui.window().is_visible() {
                            let _ = ui.window().hide();
                            trim_own_memory();
                        } else {
                            let _ = ui.window().show();
                            let _ = ui.window().set_minimized(false);
                        }
                    }
                }
            }
        }
    );

    // Close button always hides to tray (never exits)
    let ui_handle_close = ui.as_weak();
    ui.on_close_app(move || {
        if let Some(ui) = ui_handle_close.upgrade() {
            // Always hide to tray (don't exit)
            let _ = ui.window().hide();
            // Trim memory when hiding to tray for minimal idle footprint
            trim_own_memory();
        }
    });
    
    // Keep tray timer alive
    let _tray_timer_keeper = tray_timer;

    // 11. DWM Transparency Fix
    let ui_handle_dwm = ui.as_weak();
    slint::Timer::single_shot(std::time::Duration::from_millis(100), move || {
        let _ = ui_handle_dwm.upgrade_in_event_loop(|_| {
            unsafe {
                use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
                use windows::Win32::Foundation::HWND;
                
                #[repr(C)]
                #[allow(non_snake_case)]
                struct MARGINS { cxLeftWidth: i32, cxRightWidth: i32, cyTopHeight: i32, cyBottomHeight: i32 }
                
                #[link(name = "dwmapi")]
                extern "system" {
                    fn DwmExtendFrameIntoClientArea(hwnd: HWND, margins: *const MARGINS) -> windows::core::HRESULT;
                }

                let hwnd = GetForegroundWindow(); 
                if !hwnd.0.is_null() {
                    let margins = MARGINS { cxLeftWidth: -1, cxRightWidth: -1, cyTopHeight: -1, cyBottomHeight: -1 };
                    let _ = DwmExtendFrameIntoClientArea(hwnd, &margins);
                }
            }
        });
    });

    ui.run()
}
