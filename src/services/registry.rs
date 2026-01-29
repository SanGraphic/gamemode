use windows::core::{PCWSTR, HSTRING};
use windows::Win32::System::Registry::{
    RegOpenKeyExW, RegSetValueExW, RegCloseKey, RegQueryValueExW, RegCreateKeyExW,
    HKEY, HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER, KEY_WRITE, KEY_READ, REG_DWORD,
    REG_OPTION_NON_VOLATILE, REG_CREATE_KEY_DISPOSITION,
};
use std::mem::size_of;
use std::sync::Mutex;

/// RegistryService - 1:1 port of RegistryService.cs
/// Stores original values before modifying, exactly like C# implementation
pub struct RegistryService {
    // 1:1 with C# private object fields
    original_win32_priority_separation: Mutex<Option<u32>>,
    original_auto_game_mode_enabled: Mutex<Option<u32>>,
    original_priority: Mutex<Option<u32>>,
    original_gpu_priority: Mutex<Option<u32>>,
    original_auto_restart_shell: Mutex<Option<u32>>,
}

impl RegistryService {
    pub fn new() -> Self {
        Self {
            original_win32_priority_separation: Mutex::new(None),
            original_auto_game_mode_enabled: Mutex::new(None),
            original_priority: Mutex::new(None),
            original_gpu_priority: Mutex::new(None),
            original_auto_restart_shell: Mutex::new(None),
        }
    }

    /// 1:1 port of ApplyTweaks() from RegistryService.cs
    pub fn apply_tweaks(&self) {
        unsafe {
            // 1. PriorityControl - Win32PrioritySeparation
            // C#: Store original, then set to 38
            {
                let original = Self::read_dword(
                    HKEY_LOCAL_MACHINE, 
                    "SYSTEM\\CurrentControlSet\\Control\\PriorityControl", 
                    "Win32PrioritySeparation"
                );
                *self.original_win32_priority_separation.lock().unwrap() = original;
                
                Self::set_dword(
                    HKEY_LOCAL_MACHINE, 
                    "SYSTEM\\CurrentControlSet\\Control\\PriorityControl", 
                    "Win32PrioritySeparation", 
                    38
                );
            }

            // 2. GameBar - AutoGameModeEnabled & AllowAutoGameMode
            // C#: Store original AutoGameModeEnabled, then set both to 1
            {
                let original = Self::read_dword(
                    HKEY_CURRENT_USER, 
                    "Software\\Microsoft\\GameBar", 
                    "AutoGameModeEnabled"
                );
                *self.original_auto_game_mode_enabled.lock().unwrap() = original;
                
                Self::set_dword(HKEY_CURRENT_USER, "Software\\Microsoft\\GameBar", "AutoGameModeEnabled", 1);
                Self::set_dword(HKEY_CURRENT_USER, "Software\\Microsoft\\GameBar", "AllowAutoGameMode", 1);
            }

            // 3. Multimedia SystemProfile Tasks Games - Priority & GPU Priority
            // C#: Store originals, then set Priority=6, GPU Priority=8
            {
                let original_priority = Self::read_dword(
                    HKEY_LOCAL_MACHINE, 
                    "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Multimedia\\SystemProfile\\Tasks\\Games", 
                    "Priority"
                );
                *self.original_priority.lock().unwrap() = original_priority;
                
                let original_gpu = Self::read_dword(
                    HKEY_LOCAL_MACHINE, 
                    "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Multimedia\\SystemProfile\\Tasks\\Games", 
                    "GPU Priority"
                );
                *self.original_gpu_priority.lock().unwrap() = original_gpu;
                
                Self::set_dword(
                    HKEY_LOCAL_MACHINE, 
                    "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Multimedia\\SystemProfile\\Tasks\\Games", 
                    "Priority", 
                    6
                );
                Self::set_dword(
                    HKEY_LOCAL_MACHINE, 
                    "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Multimedia\\SystemProfile\\Tasks\\Games", 
                    "GPU Priority", 
                    8
                );
            }
        }
    }

    /// 1:1 port of UnlockPowerSettings() from RegistryService.cs
    /// Unlocks the processor performance boost mode setting in power options
    pub fn unlock_power_settings(&self) {
        unsafe {
            // C#: Set Attributes to 2 to make setting visible
            Self::set_dword(
                HKEY_LOCAL_MACHINE, 
                "SYSTEM\\CurrentControlSet\\Control\\Power\\PowerSettings\\54533251-82be-4824-96c1-47b60b740d00\\be337238-0d82-4146-a960-4f3749d470c7", 
                "Attributes", 
                2
            );
        }
    }

    /// 1:1 port of RevertTweaks() from RegistryService.cs
    /// Restores all original values that were stored before applying tweaks
    pub fn revert_tweaks(&self) {
        unsafe {
            // 1. Restore Win32PrioritySeparation
            if let Some(original) = *self.original_win32_priority_separation.lock().unwrap() {
                Self::set_dword(
                    HKEY_LOCAL_MACHINE, 
                    "SYSTEM\\CurrentControlSet\\Control\\PriorityControl", 
                    "Win32PrioritySeparation", 
                    original
                );
            }

            // 2. Restore AutoGameModeEnabled
            if let Some(original) = *self.original_auto_game_mode_enabled.lock().unwrap() {
                Self::set_dword(
                    HKEY_CURRENT_USER, 
                    "Software\\Microsoft\\GameBar", 
                    "AutoGameModeEnabled", 
                    original
                );
            }

            // 3. Restore Priority and GPU Priority
            if let Some(original) = *self.original_priority.lock().unwrap() {
                Self::set_dword(
                    HKEY_LOCAL_MACHINE, 
                    "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Multimedia\\SystemProfile\\Tasks\\Games", 
                    "Priority", 
                    original
                );
            }
            
            if let Some(original) = *self.original_gpu_priority.lock().unwrap() {
                Self::set_dword(
                    HKEY_LOCAL_MACHINE, 
                    "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Multimedia\\SystemProfile\\Tasks\\Games", 
                    "GPU Priority", 
                    original
                );
            }
        }
    }

    /// 1:1 port of DisableAutoRestartShell() from RegistryService.cs
    pub fn disable_auto_restart_shell(&self) {
        unsafe {
            // Store original value first
            let original = Self::read_dword(
                HKEY_LOCAL_MACHINE, 
                "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Winlogon", 
                "AutoRestartShell"
            );
            *self.original_auto_restart_shell.lock().unwrap() = original;
            
            // Set to 0 to disable
            Self::set_dword(
                HKEY_LOCAL_MACHINE, 
                "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Winlogon", 
                "AutoRestartShell", 
                0
            );
        }
    }

    /// 1:1 port of EnableAutoRestartShell() from RegistryService.cs
    pub fn enable_auto_restart_shell(&self) {
        unsafe {
            // Restore original value, or default to 1 if no original stored
            let value = self.original_auto_restart_shell.lock().unwrap().unwrap_or(1);
            
            Self::set_dword(
                HKEY_LOCAL_MACHINE, 
                "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Winlogon", 
                "AutoRestartShell", 
                value
            );
        }
    }

    // ========================================================================
    // Helper functions for registry operations
    // ========================================================================

    /// Read a DWORD value from registry
    unsafe fn read_dword(root: HKEY, subkey: &str, value_name: &str) -> Option<u32> {
        let mut key_handle = HKEY::default();
        let subkey_w = HSTRING::from(subkey);
        
        if RegOpenKeyExW(root, PCWSTR(subkey_w.as_ptr()), 0, KEY_READ, &mut key_handle).is_ok() {
            let value_w = HSTRING::from(value_name);
            let mut data: u32 = 0;
            let mut data_size: u32 = size_of::<u32>() as u32;
            
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

    /// Set a DWORD value in registry (creates key if needed)
    unsafe fn set_dword(root: HKEY, subkey: &str, value_name: &str, data: u32) {
        let mut key_handle = HKEY::default();
        let subkey_w = HSTRING::from(subkey);
        
        // Try to open existing key first
        let open_result = RegOpenKeyExW(root, PCWSTR(subkey_w.as_ptr()), 0, KEY_WRITE, &mut key_handle);
        
        if open_result.is_ok() {
            let value_w = HSTRING::from(value_name);
            let data_bytes = std::slice::from_raw_parts(&data as *const _ as *const u8, size_of::<u32>());
            
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
            let mut disposition: REG_CREATE_KEY_DISPOSITION = REG_CREATE_KEY_DISPOSITION::default();
            if RegCreateKeyExW(
                root,
                PCWSTR(subkey_w.as_ptr()),
                0,
                None,
                REG_OPTION_NON_VOLATILE,
                KEY_WRITE,
                None,
                &mut key_handle,
                Some(&mut disposition),
            ).is_ok() {
                let value_w = HSTRING::from(value_name);
                let data_bytes = std::slice::from_raw_parts(&data as *const _ as *const u8, size_of::<u32>());
                
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
