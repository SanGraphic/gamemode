use windows::core::{PCWSTR, HSTRING, PWSTR};
use windows::Win32::System::Registry::{
    RegOpenKeyExW, RegSetValueExW, RegCloseKey, RegDeleteValueW, RegEnumKeyExW,
    RegCreateKeyExW, HKEY, HKEY_LOCAL_MACHINE, KEY_WRITE, KEY_READ, REG_DWORD,
    REG_OPTION_NON_VOLATILE, REG_CREATE_KEY_DISPOSITION,
};
use std::mem::size_of;

pub struct NetworkService;

impl NetworkService {
    #[inline]
    pub fn toggle_isolation(enable: bool) {
        if enable {
            Self::disable_multicast();
            Self::disable_netbios();
        } else {
            Self::enable_multicast();
            Self::enable_netbios();
        }
    }

    /// C# uses Registry.LocalMachine.CreateSubKey() which creates if not exists
    fn disable_multicast() {
        unsafe {
            let mut key_handle = HKEY::default();
            let subkey = HSTRING::from("SOFTWARE\\Policies\\Microsoft\\Windows NT\\DNSClient");
            let mut disposition = REG_CREATE_KEY_DISPOSITION::default();
            
            // CreateSubKey in C# creates the key if it doesn't exist
            if RegCreateKeyExW(
                HKEY_LOCAL_MACHINE,
                PCWSTR(subkey.as_ptr()),
                0,
                None,
                REG_OPTION_NON_VOLATILE,
                KEY_WRITE,
                None,
                &mut key_handle,
                Some(&mut disposition),
            ).is_ok() {
                let value_name = HSTRING::from("EnableMulticast");
                let data = 0u32;
                let data_bytes = std::slice::from_raw_parts(&data as *const _ as *const u8, size_of::<u32>());
                let _ = RegSetValueExW(key_handle, PCWSTR(value_name.as_ptr()), 0, REG_DWORD, Some(data_bytes));
                let _ = RegCloseKey(key_handle);
            }
        }
    }

    fn enable_multicast() {
        unsafe {
            let mut key_handle = HKEY::default();
            let subkey = HSTRING::from("SOFTWARE\\Policies\\Microsoft\\Windows NT\\DNSClient");
            
            if RegOpenKeyExW(HKEY_LOCAL_MACHINE, PCWSTR(subkey.as_ptr()), 0, KEY_WRITE, &mut key_handle).is_ok() {
                let value_name = HSTRING::from("EnableMulticast");
                let _ = RegDeleteValueW(key_handle, PCWSTR(value_name.as_ptr()));
                let _ = RegCloseKey(key_handle);
            }
        }
    }

    fn disable_netbios() {
        Self::set_netbios_option(2); // 2 = Disable
    }

    fn enable_netbios() {
        Self::set_netbios_option(0); // 0 = Default (enable)
    }

    /// Optimized: Single pass through all NetBT interfaces
    fn set_netbios_option(value: u32) {
        unsafe {
            let mut root_key = HKEY::default();
            let subkey = HSTRING::from("SYSTEM\\CurrentControlSet\\Services\\NetBT\\Parameters\\Interfaces");
            
            if RegOpenKeyExW(HKEY_LOCAL_MACHINE, PCWSTR(subkey.as_ptr()), 0, KEY_READ, &mut root_key).is_ok() {
                let value_name = HSTRING::from("NetbiosOptions");
                let data_bytes = std::slice::from_raw_parts(&value as *const _ as *const u8, size_of::<u32>());
                
                let mut index = 0u32;
                let mut name_buf = [0u16; 256];
                
                loop {
                    let mut name_len = 256u32;
                    
                    if RegEnumKeyExW(
                        root_key, 
                        index, 
                        PWSTR(name_buf.as_mut_ptr()), 
                        &mut name_len, 
                        None, 
                        PWSTR::null(), 
                        None,
                        None
                    ).is_err() {
                        break;
                    }
                    
                    // Open subkey directly using the enumerated name
                    let mut sub_key = HKEY::default();
                    if RegOpenKeyExW(root_key, PWSTR(name_buf.as_mut_ptr()), 0, KEY_WRITE, &mut sub_key).is_ok() {
                        let _ = RegSetValueExW(sub_key, PCWSTR(value_name.as_ptr()), 0, REG_DWORD, Some(data_bytes));
                        let _ = RegCloseKey(sub_key);
                    }
                    
                    index += 1;
                }
                
                let _ = RegCloseKey(root_key);
            }
        }
    }
}
