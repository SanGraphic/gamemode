use crate::services::{
    registry::RegistryService,
    power::PowerService,
    detector::GameDetector,
    windows::WindowsServiceManager,
    memory::MemoryService,
    network::NetworkService,
    process::ProcessService,
    options::GameModeOptions,
};
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Registry::*;
use windows::core::PCWSTR;
use std::sync::Mutex;
use std::thread::{self, JoinHandle};

/// GameModeService - 1:1 port of GameModeService.cs
/// Optimized for minimal resource usage
pub struct GameModeService {
    power: PowerService,
    registry: RegistryService,
    suspended_shell_ux_pids: Mutex<Vec<u32>>,
    // 1:1 with C#: Track stopped services for proper restore
    stopped_services: Mutex<Vec<String>>,
    // 1:1 with C#: Track if network isolation was enabled so we always disable on exit
    network_isolated: Mutex<bool>,
}

// ============================================================================
// PROCESS LISTS - EXACT 1:1 FROM C# SOURCE (static, zero allocation)
// ============================================================================

static BROWSERS: &[&str] = &[
    "chrome", "firefox", "msedge", "brave", "opera", "vivaldi", "thorium"
];

static LAUNCHERS: &[&str] = &[
    "epicgameslauncher", "battle.net", "origin", "gog galaxy"
];

static SHELL_UX: &[&str] = &[
    "SearchHost", "SearchApp", "TextInputHost", "LockApp", 
    "MoNotificationUx", "ShellExperienceHost", "StartMenuExperienceHost"
];

static START_MENU_REPLACEMENTS: &[&str] = &[
    "StartAllBackX64", "StartAllBack", "OpenShellMenu", "ClassicStartMenu"
];

static BLOATWARE: &[&str] = &[
    "smartscreen", "Microsoft.Windows.SmartScreen", "Cortana", 
    "PhoneExperienceHost", "CrossDeviceResume", "CrossDeviceService",
    "Widgets", "WidgetService", "Mousocoreworker", "Microsoft.Media.Player",
    "OneDrive", "Dropbox", "GoogleDriveFS", 
    "Teams", "Skype", "GameBar", "GameBarPresenceWriter", "YourPhone",
    "nvcontainer", "NVDisplay.Container", "NVIDIA Share", 
    "NVIDIA Web Helper", "NVIDIA Overlay"
];

static PERIPHERALS: &[&str] = &[
    "iCue", "lghub_agent", "Razer Synapse Service", "ArmouryCrate.Service",
    "Razer Central", "Razer Synapse 3", "LGHUB", "Lghub_updater"
];

impl GameModeService {
    pub fn new() -> Self {
        Self {
            power: PowerService::new(),
            registry: RegistryService::new(),
            suspended_shell_ux_pids: Mutex::new(Vec::with_capacity(8)),
            stopped_services: Mutex::new(Vec::with_capacity(16)),
            network_isolated: Mutex::new(false),
        }
    }

    /// Enable game mode - Optimized parallel version
    pub fn enable_game_mode(&mut self, options: &GameModeOptions) {
        // Step 1: Detect fullscreen game (for focus later) - run early
        let detected_game = if options.suspend_explorer {
            GameDetector::detect_fullscreen_game()
        } else {
            None
        };
        
        // Step 2-4: Registry and power (fast, do first on main thread)
        self.registry.unlock_power_settings();
        self.registry.apply_tweaks();
        
        let is_desktop = GameDetector::is_desktop();
        if is_desktop {
            self.power.set_high_performance();
        } else {
            self.power.optimize_laptop_boost();
        }

        // Step 5: Explorer handling (if enabled)
        if options.suspend_explorer {
            ProcessService::kill_processes(START_MENU_REPLACEMENTS);
            self.registry.disable_auto_restart_shell();
            ProcessService::kill_process("explorer");
            
            if let Some((_pid, hwnd)) = detected_game {
                GameDetector::focus_window(hwnd);
            }
        }

        // Capture options for threads
        let suspend_browsers = options.suspend_browsers;
        let suspend_launchers = options.suspend_launchers;
        let isolate_network = options.isolate_network;

        // Parallel execution - minimize thread count
        let mut handles: Vec<JoinHandle<Vec<String>>> = Vec::with_capacity(3);
        
        // Thread 1: Services (heavy operation) - returns stopped services list
        // 1:1 with C#: Track which services were actually stopped
        handles.push(thread::spawn(|| {
            WindowsServiceManager::stop_optimization_services()
        }));
        
        // Thread 2: Memory flush (returns empty vec, just for consistent join)
        handles.push(thread::spawn(|| {
            MemoryService::flush_memory();
            Vec::new()
        }));
        
        // Thread 3: Network (only if needed)
        if isolate_network {
            handles.push(thread::spawn(|| {
                NetworkService::toggle_isolation(true);
                Vec::new()
            }));
            // 1:1 with C#: Track that we enabled network isolation
            if let Ok(mut guard) = self.network_isolated.lock() {
                *guard = true;
            }
        }

        // Main thread: Process operations (most critical for responsiveness)
        // Suspend Shell UX first
        let shell_pids = ProcessService::suspend_processes(SHELL_UX);
        
        // Build kill list efficiently (no allocation if sizes known)
        let kill_count = START_MENU_REPLACEMENTS.len() 
            + BLOATWARE.len() 
            + PERIPHERALS.len()
            + if suspend_browsers { BROWSERS.len() } else { 0 }
            + if suspend_launchers { LAUNCHERS.len() } else { 0 };
        
        let mut all_to_kill: Vec<&str> = Vec::with_capacity(kill_count);
        all_to_kill.extend_from_slice(START_MENU_REPLACEMENTS);
        if suspend_browsers {
            all_to_kill.extend_from_slice(BROWSERS);
        }
        all_to_kill.extend_from_slice(BLOATWARE);
        all_to_kill.extend_from_slice(PERIPHERALS);
        if suspend_launchers {
            all_to_kill.extend_from_slice(LAUNCHERS);
        }
        
        ProcessService::kill_processes(&all_to_kill);
        
        // Store suspended PIDs
        if let Ok(mut guard) = self.suspended_shell_ux_pids.lock() {
            *guard = shell_pids;
        }
        
        // Wait for background threads and collect stopped services
        for handle in handles {
            if let Ok(result) = handle.join() {
                if !result.is_empty() {
                    if let Ok(mut guard) = self.stopped_services.lock() {
                        guard.extend(result);
                    }
                }
            }
        }
    }

    /// Disable game mode - Optimized parallel version
    /// 1:1 with C# DisableGameModeAsync
    pub fn disable_game_mode(&self, options: &GameModeOptions) {
        let mut handles: Vec<JoinHandle<()>> = Vec::with_capacity(4);
        
        // Thread 1: Restore explorer (if needed)
        // 1:1 with C#: RestartExplorer() checks if explorer is running first
        if options.suspend_explorer {
            handles.push(thread::spawn(|| {
                ProcessService::restart_explorer();
            }));
        }
        
        // Thread 2: Restore services - 1:1 with C#: Only restore services we actually stopped
        let services_to_restore = self.stopped_services.lock()
            .map(|g| g.clone())
            .unwrap_or_default();
        
        handles.push(thread::spawn(move || {
            WindowsServiceManager::restore_services(&services_to_restore);
        }));
        
        // Thread 3: Resume Shell UX processes
        let pids = self.suspended_shell_ux_pids.lock()
            .map(|g| g.clone())
            .unwrap_or_default();
        
        handles.push(thread::spawn(move || {
            ProcessService::resume_processes_by_pid(&pids);
            ProcessService::resume_processes(SHELL_UX);
        }));
        
        // Thread 4: Network - 1:1 with C#: Always disable if it was enabled
        // C# code: await _networkService.ToggleNetworkIsolationAsync(false);
        // The C# always calls this in DisableGameModeAsync
        let was_isolated = self.network_isolated.lock()
            .map(|g| *g)
            .unwrap_or(false);
        
        if was_isolated {
            handles.push(thread::spawn(|| {
                NetworkService::toggle_isolation(false);
            }));
        }
        
        // Main thread: Registry operations (fast)
        self.registry.revert_tweaks();
        self.registry.enable_auto_restart_shell();
        
        // Power revert
        if GameDetector::is_desktop() {
            self.power.revert_power_plan();
        } else {
            self.power.revert_laptop_boost();
        }
        
        // Clear state
        if let Ok(mut guard) = self.suspended_shell_ux_pids.lock() {
            guard.clear();
        }
        if let Ok(mut guard) = self.stopped_services.lock() {
            guard.clear();
        }
        if let Ok(mut guard) = self.network_isolated.lock() {
            *guard = false;
        }
        
        // Wait for all threads
        for handle in handles {
            let _ = handle.join();
        }
    }

    #[inline]
    pub fn detect_game(&self) -> Option<(u32, HWND)> {
        GameDetector::detect_fullscreen_game()
    }
    
    /// Enable MPO (delete OverlayTestMode) and set OverlayMinFPS=0
    pub fn set_mpo_enabled() {
        let dwm_path = r"SOFTWARE\Microsoft\Windows\Dwm";
        Self::delete_registry_value(dwm_path, "OverlayTestMode");
        Self::set_registry_dword(dwm_path, "OverlayMinFPS", 0);
        println!("[GameMode] MPO enabled + OverlayMinFPS=0");
    }
    
    /// Disable MPO (OverlayTestMode=5)
    pub fn set_mpo_disabled() {
        let dwm_path = r"SOFTWARE\Microsoft\Windows\Dwm";
        Self::set_registry_dword(dwm_path, "OverlayTestMode", 5);
        println!("[GameMode] MPO disabled");
    }
    
    #[allow(dead_code)]
    fn get_registry_dword(path: &str, value_name: &str) -> Option<u32> {
        unsafe {
            let path_wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
            let value_wide: Vec<u16> = value_name.encode_utf16().chain(std::iter::once(0)).collect();
            
            let mut hkey = HKEY::default();
            if RegOpenKeyExW(HKEY_LOCAL_MACHINE, PCWSTR(path_wide.as_ptr()), 0, KEY_READ, &mut hkey).is_err() {
                return None;
            }
            
            let mut data: u32 = 0;
            let mut data_size = std::mem::size_of::<u32>() as u32;
            let mut value_type = REG_DWORD;
            
            let result = RegQueryValueExW(
                hkey,
                PCWSTR(value_wide.as_ptr()),
                None,
                Some(&mut value_type),
                Some(std::ptr::addr_of_mut!(data) as *mut u8),
                Some(&mut data_size),
            );
            
            let _ = RegCloseKey(hkey);
            
            if result.is_ok() {
                Some(data)
            } else {
                None
            }
        }
    }
    
    fn set_registry_dword(path: &str, value_name: &str, data: u32) {
        unsafe {
            let path_wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
            let value_wide: Vec<u16> = value_name.encode_utf16().chain(std::iter::once(0)).collect();
            
            let mut hkey = HKEY::default();
            if RegCreateKeyExW(
                HKEY_LOCAL_MACHINE,
                PCWSTR(path_wide.as_ptr()),
                0,
                None,
                REG_OPTION_NON_VOLATILE,
                KEY_WRITE,
                None,
                &mut hkey,
                None,
            ).is_err() {
                return;
            }
            
            let _ = RegSetValueExW(
                hkey,
                PCWSTR(value_wide.as_ptr()),
                0,
                REG_DWORD,
                Some(&data.to_le_bytes()),
            );
            
            let _ = RegCloseKey(hkey);
        }
    }
    
    fn delete_registry_value(path: &str, value_name: &str) {
        unsafe {
            let path_wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
            let value_wide: Vec<u16> = value_name.encode_utf16().chain(std::iter::once(0)).collect();
            
            let mut hkey = HKEY::default();
            if RegOpenKeyExW(HKEY_LOCAL_MACHINE, PCWSTR(path_wide.as_ptr()), 0, KEY_WRITE, &mut hkey).is_err() {
                return;
            }
            
            let _ = RegDeleteValueW(hkey, PCWSTR(value_wide.as_ptr()));
            let _ = RegCloseKey(hkey);
        }
    }
}
