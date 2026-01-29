//! ReviOS Playbook Port - Advanced system tweaks
//! Saves original state before applying and restores on disable

use std::collections::HashMap;
use std::sync::Mutex;
use once_cell::sync::Lazy;
use windows::Win32::System::Registry::*;
use windows::Win32::System::Services::*;
use windows::core::{PCWSTR, HSTRING};

/// Stores original values to restore later
static ORIGINAL_STATE: Lazy<Mutex<OriginalState>> = Lazy::new(|| Mutex::new(OriginalState::default()));

#[derive(Default)]
struct OriginalState {
    registry_values: HashMap<String, Option<RegistryValue>>,
    /// Stores (service_name, original_startup_type, was_running)
    service_states: HashMap<String, (u32, bool)>,
    applied: bool,
}

#[derive(Clone)]
struct RegistryValue {
    data: Vec<u8>,
    value_type: u32,
}

/// Services to disable during game mode (ReviOS style)
const SERVICES_TO_DISABLE: &[&str] = &[
    "DiagTrack",           // Telemetry
    "WerSvc",              // Windows Error Reporting
    "DPS",                 // Diagnostic Policy Service
    "WdiServiceHost",      // Diagnostic Service Host
    "WdiSystemHost",       // Diagnostic System Host
    "PcaSvc",              // Program Compatibility Assistant
    "wisvc",               // Windows Insider Service
    "WSearch",             // Windows Search (heavy indexing)
    "SysMain",             // Superfetch/Prefetch
    "FontCache",           // Font Cache
    "Themes",              // Themes service
    "TabletInputService",  // Touch Keyboard
    "CDPSvc",              // Connected Devices Platform
    "CDPUserSvc",          // Connected Devices Platform User Service
    "MapsBroker",          // Maps Broker
    "lfsvc",               // Geolocation Service
    "WbioSrvc",            // Biometric Service
    "iphlpsvc",            // IP Helper (IPv6 transition)
];

/// Registry tweaks to apply
struct RegistryTweak {
    path: &'static str,
    value_name: &'static str,
    data: u32,
}

const REGISTRY_TWEAKS: &[RegistryTweak] = &[
    // === Performance Tweaks ===
    // Disable VBS/HVCI for gaming performance
    RegistryTweak { path: r"SYSTEM\CurrentControlSet\Control\DeviceGuard", value_name: "EnableVirtualizationBasedSecurity", data: 0 },
    RegistryTweak { path: r"SYSTEM\CurrentControlSet\Control\DeviceGuard\Scenarios\HypervisorEnforcedCodeIntegrity", value_name: "Enabled", data: 0 },
    
    // Disable Spectre/Meltdown mitigations (performance boost)
    RegistryTweak { path: r"SYSTEM\CurrentControlSet\Control\Session Manager\Memory Management", value_name: "FeatureSettingsOverride", data: 3 },
    RegistryTweak { path: r"SYSTEM\CurrentControlSet\Control\Session Manager\Memory Management", value_name: "FeatureSettingsOverrideMask", data: 3 },
    
    // Faster shutdown
    RegistryTweak { path: r"SYSTEM\CurrentControlSet\Control", value_name: "WaitToKillServiceTimeout", data: 1500 },
    
    // Disable automatic maintenance
    RegistryTweak { path: r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Schedule\Maintenance", value_name: "MaintenanceDisabled", data: 1 },
    
    // === Telemetry Disabled ===
    RegistryTweak { path: r"SOFTWARE\Policies\Microsoft\Windows\DataCollection", value_name: "AllowTelemetry", data: 0 },
    RegistryTweak { path: r"SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\DataCollection", value_name: "AllowTelemetry", data: 0 },
    
    // Disable experimentation
    RegistryTweak { path: r"SOFTWARE\Microsoft\PolicyManager\current\device\System", value_name: "AllowExperimentation", data: 0 },
    RegistryTweak { path: r"SOFTWARE\Policies\Microsoft\Windows\PreviewBuilds", value_name: "EnableConfigFlighting", data: 0 },
    
    // === Explorer Performance ===
    // Disable folder type auto-discovery
    RegistryTweak { path: r"SOFTWARE\Classes\Local Settings\Software\Microsoft\Windows\Shell\Bags\AllFolders\Shell", value_name: "FolderType", data: 0 }, // Will handle as string
    
    // Disable search indexing in explorer
    RegistryTweak { path: r"SOFTWARE\Policies\Microsoft\Windows\Windows Search", value_name: "AllowCortana", data: 0 },
    
    // === Network Optimizations ===
    // Disable Nagle's algorithm for lower latency
    RegistryTweak { path: r"SOFTWARE\Microsoft\MSMQ\Parameters", value_name: "TCPNoDelay", data: 1 },
    
    // === GPU Optimizations ===
    // Disable GPU power saving
    RegistryTweak { path: r"SYSTEM\CurrentControlSet\Control\Power\PowerSettings\54533251-82be-4824-96c1-47b60b740d00\be337238-0d82-4146-a960-4f3749d470c7", value_name: "Attributes", data: 2 },
    
    // Hardware accelerated GPU scheduling (if supported)
    RegistryTweak { path: r"SYSTEM\CurrentControlSet\Control\GraphicsDrivers", value_name: "HwSchMode", data: 2 },
    
    // === Multimedia/Gaming ===
    // Multimedia Class Scheduler - prioritize games
    RegistryTweak { path: r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile", value_name: "SystemResponsiveness", data: 0 },
    RegistryTweak { path: r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile", value_name: "NetworkThrottlingIndex", data: 0xFFFFFFFF },
    
    // Game priority
    RegistryTweak { path: r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile\Tasks\Games", value_name: "Priority", data: 6 },
    RegistryTweak { path: r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile\Tasks\Games", value_name: "Scheduling Category", data: 2 }, // Will handle as string
    RegistryTweak { path: r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile\Tasks\Games", value_name: "SFIO Priority", data: 3 }, // Will handle as string
    
    // === Power Tweaks ===
    // Disable power throttling
    RegistryTweak { path: r"SYSTEM\CurrentControlSet\Control\Power\PowerThrottling", value_name: "PowerThrottlingOff", data: 1 },
];

pub struct ReviTweaksService;

impl ReviTweaksService {
    /// Apply all ReviOS-style tweaks, saving original state first
    pub fn enable() {
        let mut state = ORIGINAL_STATE.lock().unwrap();
        
        if state.applied {
            return; // Already applied
        }
        
        println!("[ReviTweaks] Saving original state and applying tweaks...");
        
        // Save and modify services - both registry AND actually stop them
        for service_name in SERVICES_TO_DISABLE {
            // Get original startup type from registry
            let original_startup = Self::get_service_startup_registry(service_name).unwrap_or(3);
            
            // Check if service is currently running
            let was_running = Self::is_service_running(service_name);
            
            // Save original state
            state.service_states.insert(service_name.to_string(), (original_startup, was_running));
            
            // Set startup type to Disabled (4) in registry
            Self::set_service_startup_registry(service_name, 4);
            
            // Actually STOP the service if it's running
            if was_running {
                Self::stop_service(service_name);
            }
        }
        
        // Save and modify registry values
        for tweak in REGISTRY_TWEAKS {
            let key = format!("HKLM\\{}\\{}", tweak.path, tweak.value_name);
            
            // Save original value
            let original = Self::get_registry_dword(tweak.path, tweak.value_name);
            state.registry_values.insert(key.clone(), original.map(|d| RegistryValue {
                data: d.to_le_bytes().to_vec(),
                value_type: REG_DWORD.0,
            }));
            
            // Apply new value
            Self::set_registry_dword(tweak.path, tweak.value_name, tweak.data);
        }
        
        // Apply string registry values
        Self::apply_string_tweaks(&mut state);
        
        state.applied = true;
        println!("[ReviTweaks] Applied {} service changes and {} registry tweaks", 
                 state.service_states.len(), state.registry_values.len());
    }
    
    /// Restore all original values
    pub fn disable() {
        let mut state = ORIGINAL_STATE.lock().unwrap();
        
        if !state.applied {
            return; // Nothing to restore
        }
        
        println!("[ReviTweaks] Restoring original state...");
        
        // Restore services - both registry AND restart if they were running
        for (service_name, (original_startup, was_running)) in &state.service_states {
            // Restore original startup type in registry
            Self::set_service_startup_registry(service_name, *original_startup);
            
            // Restart service if it was running before
            if *was_running {
                Self::start_service(service_name);
            }
        }
        
        // Restore registry values
        for (key, original_value) in &state.registry_values {
            // Parse key back to path and value name
            if let Some((path, value_name)) = key.strip_prefix("HKLM\\").and_then(|k| {
                k.rsplit_once('\\')
            }) {
                if let Some(reg_val) = original_value {
                    if reg_val.value_type == REG_DWORD.0 && reg_val.data.len() >= 4 {
                        let data = u32::from_le_bytes([reg_val.data[0], reg_val.data[1], reg_val.data[2], reg_val.data[3]]);
                        Self::set_registry_dword(path, value_name, data);
                    }
                } else {
                    // Value didn't exist before, delete it
                    Self::delete_registry_value(path, value_name);
                }
            }
        }
        
        // Restore string values
        Self::restore_string_tweaks(&state);
        
        state.service_states.clear();
        state.registry_values.clear();
        state.applied = false;
        
        println!("[ReviTweaks] Restored original state");
    }
    
    /// Check if tweaks are currently applied
    #[allow(dead_code)]
    pub fn is_applied() -> bool {
        ORIGINAL_STATE.lock().unwrap().applied
    }
    
    fn apply_string_tweaks(state: &mut OriginalState) {
        // FolderType = NotSpecified (string value)
        let folder_path = r"SOFTWARE\Classes\Local Settings\Software\Microsoft\Windows\Shell\Bags\AllFolders\Shell";
        let key = format!("HKLM\\{}\\FolderType_str", folder_path);
        let original = Self::get_registry_string(folder_path, "FolderType");
        state.registry_values.insert(key, original.map(|s| RegistryValue {
            data: s.into_bytes(),
            value_type: REG_SZ.0,
        }));
        Self::set_registry_string(folder_path, "FolderType", "NotSpecified");
        
        // MMCSS Game scheduling
        let mmcss_path = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile\Tasks\Games";
        
        let key = format!("HKLM\\{}\\Scheduling Category_str", mmcss_path);
        let original = Self::get_registry_string(mmcss_path, "Scheduling Category");
        state.registry_values.insert(key, original.map(|s| RegistryValue {
            data: s.into_bytes(),
            value_type: REG_SZ.0,
        }));
        Self::set_registry_string(mmcss_path, "Scheduling Category", "High");
        
        let key = format!("HKLM\\{}\\SFIO Priority_str", mmcss_path);
        let original = Self::get_registry_string(mmcss_path, "SFIO Priority");
        state.registry_values.insert(key, original.map(|s| RegistryValue {
            data: s.into_bytes(),
            value_type: REG_SZ.0,
        }));
        Self::set_registry_string(mmcss_path, "SFIO Priority", "High");
    }
    
    fn restore_string_tweaks(state: &OriginalState) {
        for (key, original_value) in &state.registry_values {
            if key.ends_with("_str") {
                if let Some((path, value_name)) = key.strip_prefix("HKLM\\").and_then(|k| {
                    k.strip_suffix("_str").and_then(|k2| k2.rsplit_once('\\'))
                }) {
                    if let Some(reg_val) = original_value {
                        if reg_val.value_type == REG_SZ.0 {
                            let s = String::from_utf8_lossy(&reg_val.data).to_string();
                            Self::set_registry_string(path, value_name, &s);
                        }
                    } else {
                        Self::delete_registry_value(path, value_name);
                    }
                }
            }
        }
    }
    
    // ========== Service Control (SCM API) ==========
    
    /// Check if a service is currently running
    fn is_service_running(service_name: &str) -> bool {
        unsafe {
            let Ok(scm) = OpenSCManagerW(None, None, SC_MANAGER_CONNECT) else {
                return false;
            };
            
            let name_w = HSTRING::from(service_name);
            let result = if let Ok(service) = OpenServiceW(
                scm,
                PCWSTR(name_w.as_ptr()),
                SERVICE_QUERY_STATUS
            ) {
                let mut status = SERVICE_STATUS::default();
                let running = QueryServiceStatus(service, &mut status).is_ok() 
                    && status.dwCurrentState == SERVICE_RUNNING;
                let _ = CloseServiceHandle(service);
                running
            } else {
                false
            };
            
            let _ = CloseServiceHandle(scm);
            result
        }
    }
    
    /// Stop a running service
    fn stop_service(service_name: &str) -> bool {
        unsafe {
            let Ok(scm) = OpenSCManagerW(None, None, SC_MANAGER_CONNECT) else {
                return false;
            };
            
            let name_w = HSTRING::from(service_name);
            let result = if let Ok(service) = OpenServiceW(
                scm,
                PCWSTR(name_w.as_ptr()),
                SERVICE_STOP | SERVICE_QUERY_STATUS
            ) {
                let mut status = SERVICE_STATUS::default();
                let stopped = if QueryServiceStatus(service, &mut status).is_ok() 
                    && status.dwCurrentState == SERVICE_RUNNING 
                {
                    let mut new_status = SERVICE_STATUS::default();
                    ControlService(service, SERVICE_CONTROL_STOP, &mut new_status).is_ok()
                } else {
                    true // Already stopped
                };
                let _ = CloseServiceHandle(service);
                stopped
            } else {
                false
            };
            
            let _ = CloseServiceHandle(scm);
            result
        }
    }
    
    /// Start a stopped service
    fn start_service(service_name: &str) -> bool {
        unsafe {
            let Ok(scm) = OpenSCManagerW(None, None, SC_MANAGER_CONNECT) else {
                return false;
            };
            
            let name_w = HSTRING::from(service_name);
            let result = if let Ok(service) = OpenServiceW(
                scm,
                PCWSTR(name_w.as_ptr()),
                SERVICE_START | SERVICE_QUERY_STATUS
            ) {
                let mut status = SERVICE_STATUS::default();
                let started = if QueryServiceStatus(service, &mut status).is_ok() {
                    // SERVICE_STOPPED = 1
                    if status.dwCurrentState.0 == 1 {
                        StartServiceW(service, None).is_ok()
                    } else {
                        true // Already running
                    }
                } else {
                    false
                };
                let _ = CloseServiceHandle(service);
                started
            } else {
                false
            };
            
            let _ = CloseServiceHandle(scm);
            result
        }
    }
    
    // ========== Registry-based service startup type ==========
    
    fn get_service_startup_registry(service_name: &str) -> Option<u32> {
        let path = format!(r"SYSTEM\CurrentControlSet\Services\{}", service_name);
        Self::get_registry_dword(&path, "Start")
    }
    
    fn set_service_startup_registry(service_name: &str, startup: u32) {
        let path = format!(r"SYSTEM\CurrentControlSet\Services\{}", service_name);
        Self::set_registry_dword(&path, "Start", startup);
    }
    
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
    
    fn get_registry_string(path: &str, value_name: &str) -> Option<String> {
        unsafe {
            let path_wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
            let value_wide: Vec<u16> = value_name.encode_utf16().chain(std::iter::once(0)).collect();
            
            let mut hkey = HKEY::default();
            if RegOpenKeyExW(HKEY_LOCAL_MACHINE, PCWSTR(path_wide.as_ptr()), 0, KEY_READ, &mut hkey).is_err() {
                return None;
            }
            
            let mut data_size: u32 = 0;
            let mut value_type = REG_SZ;
            
            // First call to get size
            let _ = RegQueryValueExW(
                hkey,
                PCWSTR(value_wide.as_ptr()),
                None,
                Some(&mut value_type),
                None,
                Some(&mut data_size),
            );
            
            if data_size == 0 {
                let _ = RegCloseKey(hkey);
                return None;
            }
            
            let mut buffer: Vec<u16> = vec![0; (data_size / 2) as usize];
            
            let result = RegQueryValueExW(
                hkey,
                PCWSTR(value_wide.as_ptr()),
                None,
                Some(&mut value_type),
                Some(buffer.as_mut_ptr() as *mut u8),
                Some(&mut data_size),
            );
            
            let _ = RegCloseKey(hkey);
            
            if result.is_ok() {
                // Remove null terminator
                while buffer.last() == Some(&0) {
                    buffer.pop();
                }
                Some(String::from_utf16_lossy(&buffer))
            } else {
                None
            }
        }
    }
    
    fn set_registry_string(path: &str, value_name: &str, data: &str) {
        unsafe {
            let path_wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
            let value_wide: Vec<u16> = value_name.encode_utf16().chain(std::iter::once(0)).collect();
            let data_wide: Vec<u16> = data.encode_utf16().chain(std::iter::once(0)).collect();
            
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
            
            let data_bytes: Vec<u8> = data_wide.iter().flat_map(|&x| x.to_le_bytes()).collect();
            
            let _ = RegSetValueExW(
                hkey,
                PCWSTR(value_wide.as_ptr()),
                0,
                REG_SZ,
                Some(&data_bytes),
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
