//! Advanced Modules Service
//! Hardware-aware tweaks for 1% lows optimization
//! Each tweak is toggleable and only active when game mode is active

use crate::services::settings::AdvancedModuleSettings;
use windows::Win32::System::Registry::*;
use windows::core::{PCWSTR, HSTRING};
use std::sync::Mutex;

/// Stores original values before applying tweaks for proper restoration
pub struct AdvancedModulesService {
    // Core Parking original values
    original_core_parking_min: Mutex<Option<u32>>,
    original_core_parking_max: Mutex<Option<u32>>,
    
    // MMCSS original values
    original_system_responsiveness: Mutex<Option<u32>>,
    original_no_lazy_mode: Mutex<Option<u32>>,
    
    // Large Pages - track if we enabled it
    large_pages_enabled: Mutex<bool>,
    
    // HAGS original value
    original_hags_value: Mutex<Option<u32>>,
    
    // Process demotion - track demoted PIDs
    demoted_processes: Mutex<Vec<u32>>,
    
    // Bufferbloat - original TCP autotuning level
    original_autotuning_level: Mutex<Option<String>>,
}

impl AdvancedModulesService {
    pub fn new() -> Self {
        Self {
            original_core_parking_min: Mutex::new(None),
            original_core_parking_max: Mutex::new(None),
            original_system_responsiveness: Mutex::new(None),
            original_no_lazy_mode: Mutex::new(None),
            large_pages_enabled: Mutex::new(false),
            original_hags_value: Mutex::new(None),
            // Pre-allocate with reasonable capacity to avoid reallocs
            demoted_processes: Mutex::new(Vec::with_capacity(32)),
            original_autotuning_level: Mutex::new(None),
        }
    }

    /// Apply all enabled advanced modules
    pub fn enable(&self, settings: &AdvancedModuleSettings) {
        if settings.disable_core_parking {
            self.disable_core_parking();
        }
        if settings.mmcss_priority_boost {
            self.enable_mmcss_boost();
        }
        if settings.enable_large_pages {
            self.enable_large_pages();
        }
        if settings.enable_hags {
            self.enable_hags();
        }
        if settings.process_idle_demotion {
            self.enable_process_demotion();
        }
        if settings.lower_bufferbloat {
            self.enable_lower_bufferbloat();
        }
    }

    /// Restore all tweaks to original values
    pub fn disable(&self, settings: &AdvancedModuleSettings) {
        if settings.disable_core_parking {
            self.restore_core_parking();
        }
        if settings.mmcss_priority_boost {
            self.restore_mmcss();
        }
        if settings.enable_large_pages {
            self.restore_large_pages();
        }
        if settings.enable_hags {
            self.restore_hags();
        }
        if settings.process_idle_demotion {
            self.restore_process_priority();
        }
        if settings.lower_bufferbloat {
            self.restore_bufferbloat();
        }
    }

    // =========================================================================
    // 1. CORE PARKING DISABLE
    // Prevents micro-stutter from core wake latency
    // Registry: HKLM\SYSTEM\CurrentControlSet\Control\Power\PowerSettings\...
    // =========================================================================

    fn disable_core_parking(&self) {
        // Core parking is controlled via power settings
        // We set minimum parked cores to 100% (meaning no cores can park)
        // Path: 54533251-82be-4824-96c1-47b60b740d00\0cc5b647-c1df-4637-891a-dec35c318583
        
        let power_path = r"SYSTEM\CurrentControlSet\Control\Power\PowerSettings\54533251-82be-4824-96c1-47b60b740d00\0cc5b647-c1df-4637-891a-dec35c318583";
        
        // Store original values
        let original_min = Self::read_registry_dword(HKEY_LOCAL_MACHINE, power_path, "ValueMin");
        let original_max = Self::read_registry_dword(HKEY_LOCAL_MACHINE, power_path, "ValueMax");
        
        *self.original_core_parking_min.lock().unwrap() = original_min;
        *self.original_core_parking_max.lock().unwrap() = original_max;
        
        // Also need to modify the active power scheme
        // Use powercfg to set core parking to 100% (disabled)
        use std::process::Command;
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        
        // Processor performance core parking min cores (AC) - set to 100
        let _ = Command::new("powercfg")
            .args(["/setacvalueindex", "scheme_current", "sub_processor", "CPMINCORES", "100"])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
        
        // Processor performance core parking max cores (AC) - set to 100
        let _ = Command::new("powercfg")
            .args(["/setacvalueindex", "scheme_current", "sub_processor", "CPMAXCORES", "100"])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
        
        // Apply the changes
        let _ = Command::new("powercfg")
            .args(["/setactive", "scheme_current"])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
        
        println!("[AdvancedModules] Core parking disabled");
    }

    fn restore_core_parking(&self) {
        use std::process::Command;
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        
        // Restore default values (50% for min, 100% for max is Windows default)
        let _ = Command::new("powercfg")
            .args(["/setacvalueindex", "scheme_current", "sub_processor", "CPMINCORES", "50"])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
        
        let _ = Command::new("powercfg")
            .args(["/setacvalueindex", "scheme_current", "sub_processor", "CPMAXCORES", "100"])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
        
        let _ = Command::new("powercfg")
            .args(["/setactive", "scheme_current"])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
        
        println!("[AdvancedModules] Core parking restored");
    }

    // =========================================================================
    // 5. MMCSS PRIORITY BOOST
    // Boost Multimedia Class Scheduler Service priority for game threads
    // Registry: HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile
    // =========================================================================

    fn enable_mmcss_boost(&self) {
        let mmcss_path = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile";
        
        // Store original SystemResponsiveness (default is usually 20)
        let original_resp = Self::read_registry_dword(HKEY_LOCAL_MACHINE, mmcss_path, "SystemResponsiveness");
        *self.original_system_responsiveness.lock().unwrap() = original_resp;
        
        // Store original NoLazyMode
        let original_lazy = Self::read_registry_dword(HKEY_LOCAL_MACHINE, mmcss_path, "NoLazyMode");
        *self.original_no_lazy_mode.lock().unwrap() = original_lazy;
        
        // Set SystemResponsiveness to 0 (give maximum CPU to multimedia/games)
        // This means 0% of CPU is reserved for background tasks when MMCSS is active
        Self::set_registry_dword(HKEY_LOCAL_MACHINE, mmcss_path, "SystemResponsiveness", 0);
        
        // Enable NoLazyMode (1) - process MMCSS requests immediately
        Self::set_registry_dword(HKEY_LOCAL_MACHINE, mmcss_path, "NoLazyMode", 1);
        
        // Also boost the Games task specifically
        let games_path = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile\Tasks\Games";
        Self::set_registry_dword(HKEY_LOCAL_MACHINE, games_path, "Scheduling Category", 2); // High
        Self::set_registry_dword(HKEY_LOCAL_MACHINE, games_path, "SFIO Priority", 2); // High
        Self::set_registry_dword(HKEY_LOCAL_MACHINE, games_path, "Background Only", 0);
        Self::set_registry_dword(HKEY_LOCAL_MACHINE, games_path, "Clock Rate", 10000); // 1ms
        
        println!("[AdvancedModules] MMCSS priority boost enabled");
    }

    fn restore_mmcss(&self) {
        let mmcss_path = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile";
        
        // Restore SystemResponsiveness (default 20)
        let original = self.original_system_responsiveness.lock().unwrap().unwrap_or(20);
        Self::set_registry_dword(HKEY_LOCAL_MACHINE, mmcss_path, "SystemResponsiveness", original);
        
        // Restore NoLazyMode (default 0)
        let original_lazy = self.original_no_lazy_mode.lock().unwrap().unwrap_or(0);
        Self::set_registry_dword(HKEY_LOCAL_MACHINE, mmcss_path, "NoLazyMode", original_lazy);
        
        println!("[AdvancedModules] MMCSS priority restored");
    }

    // =========================================================================
    // 4. LARGE SYSTEM PAGES
    // Enable large pages for better TLB efficiency
    // Registry: HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Memory Management
    // =========================================================================

    fn enable_large_pages(&self) {
        let mem_path = r"SYSTEM\CurrentControlSet\Control\Session Manager\Memory Management";
        
        // LargeSystemCache: 1 = System favors system cache working set
        Self::set_registry_dword(HKEY_LOCAL_MACHINE, mem_path, "LargeSystemCache", 1);
        
        // LargePageMinimum - helps with large page allocation
        // Note: Actual large page support also requires SeLockMemoryPrivilege
        Self::set_registry_dword(HKEY_LOCAL_MACHINE, mem_path, "LargePageMinimum", 1);
        
        *self.large_pages_enabled.lock().unwrap() = true;
        
        println!("[AdvancedModules] Large pages enabled (requires reboot for full effect)");
    }

    fn restore_large_pages(&self) {
        if !*self.large_pages_enabled.lock().unwrap() {
            return;
        }
        
        let mem_path = r"SYSTEM\CurrentControlSet\Control\Session Manager\Memory Management";
        
        // Restore defaults
        Self::set_registry_dword(HKEY_LOCAL_MACHINE, mem_path, "LargeSystemCache", 0);
        
        *self.large_pages_enabled.lock().unwrap() = false;
        
        println!("[AdvancedModules] Large pages disabled");
    }

    // =========================================================================
    // 8. HARDWARE-ACCELERATED GPU SCHEDULING (HAGS)
    // Registry: HKLM\SYSTEM\CurrentControlSet\Control\GraphicsDrivers
    // =========================================================================

    fn enable_hags(&self) {
        let gpu_path = r"SYSTEM\CurrentControlSet\Control\GraphicsDrivers";
        
        // Store original value
        let original = Self::read_registry_dword(HKEY_LOCAL_MACHINE, gpu_path, "HwSchMode");
        *self.original_hags_value.lock().unwrap() = original;
        
        // HwSchMode: 2 = Hardware-accelerated GPU scheduling enabled
        // 1 = Enabled but not hardware-accelerated
        // 0 = Disabled
        Self::set_registry_dword(HKEY_LOCAL_MACHINE, gpu_path, "HwSchMode", 2);
        
        println!("[AdvancedModules] HAGS enabled (requires reboot)");
    }

    fn restore_hags(&self) {
        let original = *self.original_hags_value.lock().unwrap();
        
        if let Some(val) = original {
            let gpu_path = r"SYSTEM\CurrentControlSet\Control\GraphicsDrivers";
            Self::set_registry_dword(HKEY_LOCAL_MACHINE, gpu_path, "HwSchMode", val);
            println!("[AdvancedModules] HAGS restored to previous value");
        }
    }

    // =========================================================================
    // 11. PROCESS IDLE DEMOTION
    // Set non-essential processes to idle priority during game mode
    // =========================================================================

    fn enable_process_demotion(&self) {
        use windows::Win32::System::Threading::{
            OpenProcess, SetPriorityClass, PROCESS_SET_INFORMATION, PROCESS_QUERY_LIMITED_INFORMATION,
            IDLE_PRIORITY_CLASS,
        };
        use windows::Win32::System::Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, Process32First, Process32Next, PROCESSENTRY32, TH32CS_SNAPPROCESS,
        };
        use windows::Win32::Foundation::CloseHandle;

        // Processes to demote (background apps that shouldn't compete with games)
        const DEMOTE_PROCESSES: &[&str] = &[
            "SearchIndexer", "SecurityHealthService", "SgrmBroker",
            "compattelrunner", "MsMpEng", "NisSrv", "WmiPrvSE",
            "spoolsv", "dllhost", "backgroundTaskHost",
            "RuntimeBroker", "ApplicationFrameHost", "SystemSettings",
            "SettingSyncHost", "OneDrive", "GoogleDriveFS", "Dropbox",
        ];

        let current_pid = std::process::id();
        // Pre-allocate to avoid reallocs during iteration
        let mut demoted = Vec::with_capacity(32);

        unsafe {
            let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else { return };
            if snapshot.is_invalid() { return; }

            let mut entry = PROCESSENTRY32 {
                dwSize: std::mem::size_of::<PROCESSENTRY32>() as u32,
                ..Default::default()
            };

            if Process32First(snapshot, &mut entry).is_ok() {
                loop {
                    let pid = entry.th32ProcessID;
                    
                    if pid != current_pid && pid != 0 && pid != 4 {
                        let name = Self::extract_process_name(&entry.szExeFile);
                        
                        // Check if this process should be demoted
                        if DEMOTE_PROCESSES.iter().any(|&p| name.eq_ignore_ascii_case(p)) {
                            if let Ok(handle) = OpenProcess(
                                PROCESS_SET_INFORMATION | PROCESS_QUERY_LIMITED_INFORMATION,
                                false,
                                pid
                            ) {
                                if SetPriorityClass(handle, IDLE_PRIORITY_CLASS).is_ok() {
                                    demoted.push(pid);
                                }
                                let _ = CloseHandle(handle);
                            }
                        }
                    }

                    if Process32Next(snapshot, &mut entry).is_err() { break; }
                }
            }
            
            let _ = CloseHandle(snapshot);
        }

        let count = demoted.len();
        *self.demoted_processes.lock().unwrap() = demoted;
        println!("[AdvancedModules] Process idle demotion enabled ({} processes)", count);
    }

    fn restore_process_priority(&self) {
        use windows::Win32::System::Threading::{
            OpenProcess, SetPriorityClass, PROCESS_SET_INFORMATION,
            NORMAL_PRIORITY_CLASS,
        };
        use windows::Win32::Foundation::CloseHandle;

        // Take ownership to avoid holding lock during iteration
        let demoted = std::mem::take(&mut *self.demoted_processes.lock().unwrap());
        
        unsafe {
            for pid in &demoted {
                if let Ok(handle) = OpenProcess(PROCESS_SET_INFORMATION, false, *pid) {
                    let _ = SetPriorityClass(handle, NORMAL_PRIORITY_CLASS);
                    let _ = CloseHandle(handle);
                }
            }
        }
        
        // Vec is dropped here, memory freed
        println!("[AdvancedModules] Process priorities restored ({} processes)", demoted.len());
    }

    // =========================================================================
    // 12. LOWER BUFFERBLOAT
    // Disable TCP autotuning to reduce network latency spikes
    // Command: netsh int tcp set global autotuninglevel=disabled
    // =========================================================================

    fn enable_lower_bufferbloat(&self) {
        use std::process::Command;
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        
        // Get current autotuning level first
        let output = Command::new("netsh")
            .args(["int", "tcp", "show", "global"])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
        
        if let Ok(out) = output {
            let stdout = String::from_utf8_lossy(&out.stdout);
            // Parse the current autotuning level
            for line in stdout.lines() {
                if line.to_lowercase().contains("auto-tuning") || line.to_lowercase().contains("autotuning") {
                    // Extract the value (e.g., "normal", "disabled", "highlyrestricted")
                    if let Some(level) = line.split(':').nth(1) {
                        let level = level.trim().to_lowercase();
                        *self.original_autotuning_level.lock().unwrap() = Some(level);
                        break;
                    }
                }
            }
        }
        
        // Set autotuning to disabled
        let _ = Command::new("netsh")
            .args(["int", "tcp", "set", "global", "autotuninglevel=disabled"])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
        
        println!("[AdvancedModules] Bufferbloat reduction enabled (TCP autotuning disabled)");
    }

    fn restore_bufferbloat(&self) {
        use std::process::Command;
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        
        // Restore original autotuning level
        let original = self.original_autotuning_level.lock().unwrap().clone();
        let level = original.unwrap_or_else(|| "normal".to_string());
        
        let _ = Command::new("netsh")
            .args(["int", "tcp", "set", "global", &format!("autotuninglevel={}", level)])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
        
        println!("[AdvancedModules] Bufferbloat setting restored (TCP autotuning: {})", level);
    }

    // =========================================================================
    // PERMANENT TOGGLE FUNCTIONS (Can be called without game mode)
    // =========================================================================

    /// Get current TCP autotuning status
    pub fn get_bufferbloat_status() -> bool {
        use std::process::Command;
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        
        let output = Command::new("netsh")
            .args(["int", "tcp", "show", "global"])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
        
        if let Ok(out) = output {
            let stdout = String::from_utf8_lossy(&out.stdout).to_lowercase();
            // If autotuning is disabled, bufferbloat reduction is ON
            stdout.contains("disabled")
        } else {
            false
        }
    }

    /// Permanently enable bufferbloat reduction (disable TCP autotuning)
    pub fn set_bufferbloat_enabled() {
        use std::process::Command;
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        
        let _ = Command::new("netsh")
            .args(["int", "tcp", "set", "global", "autotuninglevel=disabled"])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
        
        println!("[AdvancedModules] Bufferbloat reduction permanently enabled");
    }

    /// Permanently disable bufferbloat reduction (restore TCP autotuning to normal)
    pub fn set_bufferbloat_disabled() {
        use std::process::Command;
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        
        let _ = Command::new("netsh")
            .args(["int", "tcp", "set", "global", "autotuninglevel=normal"])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
        
        println!("[AdvancedModules] Bufferbloat reduction permanently disabled (TCP autotuning normal)");
    }

    // =========================================================================
    // HELPER FUNCTIONS
    // =========================================================================

    fn extract_process_name(sz_exe_file: &[i8; 260]) -> &str {
        let len = sz_exe_file.iter().position(|&c| c == 0).unwrap_or(260);
        let bytes = unsafe { std::slice::from_raw_parts(sz_exe_file.as_ptr() as *const u8, len) };
        let name = std::str::from_utf8(bytes).unwrap_or("");
        name.strip_suffix(".exe").or_else(|| name.strip_suffix(".EXE")).unwrap_or(name)
    }

    fn read_registry_dword(root: HKEY, subkey: &str, value_name: &str) -> Option<u32> {
        unsafe {
            let mut key_handle = HKEY::default();
            let subkey_w = HSTRING::from(subkey);
            
            if RegOpenKeyExW(root, PCWSTR(subkey_w.as_ptr()), 0, KEY_READ, &mut key_handle).is_ok() {
                let value_w = HSTRING::from(value_name);
                let mut data: u32 = 0;
                let mut data_size: u32 = std::mem::size_of::<u32>() as u32;
                
                let result = RegQueryValueExW(
                    key_handle,
                    PCWSTR(value_w.as_ptr()),
                    None,
                    None,
                    Some(&mut data as *mut u32 as *mut u8),
                    Some(&mut data_size),
                );
                
                let _ = RegCloseKey(key_handle);
                
                if result.is_ok() {
                    return Some(data);
                }
            }
            None
        }
    }

    fn set_registry_dword(root: HKEY, subkey: &str, value_name: &str, data: u32) {
        unsafe {
            let mut key_handle = HKEY::default();
            let subkey_w = HSTRING::from(subkey);
            
            // Try to open existing key first
            let open_result = RegOpenKeyExW(root, PCWSTR(subkey_w.as_ptr()), 0, KEY_WRITE, &mut key_handle);
            
            if open_result.is_ok() {
                let value_w = HSTRING::from(value_name);
                let data_bytes = std::slice::from_raw_parts(&data as *const _ as *const u8, std::mem::size_of::<u32>());
                
                let _ = RegSetValueExW(
                    key_handle,
                    PCWSTR(value_w.as_ptr()),
                    0,
                    REG_DWORD,
                    Some(data_bytes),
                );
                let _ = RegCloseKey(key_handle);
            } else {
                // Try to create the key
                if RegCreateKeyExW(
                    root,
                    PCWSTR(subkey_w.as_ptr()),
                    0,
                    None,
                    REG_OPTION_NON_VOLATILE,
                    KEY_WRITE,
                    None,
                    &mut key_handle,
                    None,
                ).is_ok() {
                    let value_w = HSTRING::from(value_name);
                    let data_bytes = std::slice::from_raw_parts(&data as *const _ as *const u8, std::mem::size_of::<u32>());
                    
                    let _ = RegSetValueExW(
                        key_handle,
                        PCWSTR(value_w.as_ptr()),
                        0,
                        REG_DWORD,
                        Some(data_bytes),
                    );
                    let _ = RegCloseKey(key_handle);
                }
            }
        }
    }
}
