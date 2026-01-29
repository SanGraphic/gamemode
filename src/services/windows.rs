use windows::Win32::System::Services::{
    OpenSCManagerW, OpenServiceW, ControlService, CloseServiceHandle, StartServiceW,
    QueryServiceStatus, SC_MANAGER_CONNECT, SERVICE_STOP, SERVICE_START, 
    SERVICE_CONTROL_STOP, SERVICE_STATUS, SERVICE_QUERY_STATUS, SERVICE_RUNNING,
};
use windows::core::{PCWSTR, HSTRING};
use std::thread;
use std::sync::Mutex;

pub struct WindowsServiceManager;

impl WindowsServiceManager {
    // 1:1 List from C# WindowsServiceManager.cs (static, zero allocation)
    pub const OPTIMIZATION_SERVICES: &'static [&'static str] = &[
        "SysMain", "DiagTrack", "WSearch", "Spooler", "MapsBroker", "Fax", 
        "NvContainerLocalSystem", "NvContainerNetworkService", "NVDisplay.ContainerLocalSystem", 
        "CrossDeviceService", "wuauserv", "bits", "dosvc"
    ];

    /// Stop optimization services - Parallel with thread-safe collection
    pub fn stop_optimization_services() -> Vec<String> {
        let stopped = Mutex::new(Vec::with_capacity(Self::OPTIMIZATION_SERVICES.len()));
        
        thread::scope(|s| {
            for &name in Self::OPTIMIZATION_SERVICES {
                let stopped_ref = &stopped;
                
                s.spawn(move || {
                    if Self::stop_single_service(name) {
                        if let Ok(mut guard) = stopped_ref.lock() {
                            guard.push(name.to_string());
                        }
                    }
                });
            }
        });
        
        stopped.into_inner().unwrap_or_default()
    }

    /// Stop a single service - returns true if stopped
    #[inline]
    fn stop_single_service(name: &str) -> bool {
        unsafe {
            let Ok(scm) = OpenSCManagerW(None, None, SC_MANAGER_CONNECT) else { 
                return false; 
            };
            
            let name_w = HSTRING::from(name);
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
                    false
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

    /// Restore services - Parallel
    pub fn restore_services(service_names: &[String]) {
        thread::scope(|s| {
            for name in service_names {
                s.spawn(move || {
                    Self::start_single_service(name);
                });
            }
        });
    }

    /// Start a single service
    #[inline]
    fn start_single_service(name: &str) {
        unsafe {
            let Ok(scm) = OpenSCManagerW(None, None, SC_MANAGER_CONNECT) else { return };
            
            let name_w = HSTRING::from(name);
            if let Ok(service) = OpenServiceW(
                scm, 
                PCWSTR(name_w.as_ptr()), 
                SERVICE_START | SERVICE_QUERY_STATUS
            ) {
                let mut status = SERVICE_STATUS::default();
                if QueryServiceStatus(service, &mut status).is_ok() {
                    // SERVICE_STOPPED = 1
                    if status.dwCurrentState.0 == 1 {
                        let _ = StartServiceW(service, None);
                    }
                }
                let _ = CloseServiceHandle(service);
            }
            
            let _ = CloseServiceHandle(scm);
        }
    }
}
