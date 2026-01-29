use windows::Win32::System::Threading::{OpenProcess, PROCESS_SUSPEND_RESUME};
use windows::Win32::Foundation::{HANDLE, CloseHandle};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32First, Process32Next, PROCESSENTRY32, TH32CS_SNAPPROCESS
};
use std::process::Command;
use std::os::windows::process::CommandExt;

#[link(name = "ntdll")]
extern "system" {
    fn NtSuspendProcess(process_handle: HANDLE) -> i32;
    fn NtResumeProcess(process_handle: HANDLE) -> i32;
}

pub struct ProcessService;

impl ProcessService {
    /// Suspend processes by name - Optimized single-pass version
    /// Returns PIDs of suspended processes
    #[inline]
    pub fn suspend_processes(target_names: &[&str]) -> Vec<u32> {
        let mut suspended_pids = Vec::with_capacity(target_names.len());
        
        unsafe {
            let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else { 
                return suspended_pids; 
            };
            if snapshot.is_invalid() { return suspended_pids; }

            let mut entry = PROCESSENTRY32 {
                dwSize: std::mem::size_of::<PROCESSENTRY32>() as u32,
                ..Default::default()
            };

            if Process32First(snapshot, &mut entry).is_ok() {
                loop {
                    // Extract process name efficiently (avoid allocation when possible)
                    let name = Self::extract_process_name(&entry.szExeFile);
                    
                    // Check if this process should be suspended (case-insensitive)
                    if target_names.iter().any(|&t| t.eq_ignore_ascii_case(name)) {
                        if let Ok(handle) = OpenProcess(PROCESS_SUSPEND_RESUME, false, entry.th32ProcessID) {
                            NtSuspendProcess(handle);
                            suspended_pids.push(entry.th32ProcessID);
                            let _ = CloseHandle(handle);
                        }
                    }

                    if Process32Next(snapshot, &mut entry).is_err() { break; }
                }
            }
            let _ = CloseHandle(snapshot);
        }
        suspended_pids
    }

    /// Resume processes by name - Optimized single-pass version
    #[inline]
    pub fn resume_processes(target_names: &[&str]) {
        unsafe {
            let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else { return };
            if snapshot.is_invalid() { return; }

            let mut entry = PROCESSENTRY32 {
                dwSize: std::mem::size_of::<PROCESSENTRY32>() as u32,
                ..Default::default()
            };

            if Process32First(snapshot, &mut entry).is_ok() {
                loop {
                    let name = Self::extract_process_name(&entry.szExeFile);
                    
                    if target_names.iter().any(|&t| t.eq_ignore_ascii_case(name)) {
                        if let Ok(handle) = OpenProcess(PROCESS_SUSPEND_RESUME, false, entry.th32ProcessID) {
                            NtResumeProcess(handle);
                            let _ = CloseHandle(handle);
                        }
                    }

                    if Process32Next(snapshot, &mut entry).is_err() { break; }
                }
            }
            let _ = CloseHandle(snapshot);
        }
    }

    /// Resume processes by PID list
    #[inline]
    pub fn resume_processes_by_pid(pids: &[u32]) {
        unsafe {
            for &pid in pids {
                if let Ok(handle) = OpenProcess(PROCESS_SUSPEND_RESUME, false, pid) {
                    NtResumeProcess(handle);
                    let _ = CloseHandle(handle);
                }
            }
        }
    }

    /// Kill processes - FAST batch version using single taskkill command
    /// C# calls taskkill for each process individually twice, but batching is faster
    #[inline]
    pub fn kill_processes(target_names: &[&str]) {
        if target_names.is_empty() { return; }
        
        // Build taskkill arguments: /F /IM proc1.exe /IM proc2.exe ...
        // Capacity: "/F" + ("/IM" + "name.exe") * count
        let mut args = Vec::with_capacity(1 + target_names.len() * 2);
        args.push("/F");
        
        for name in target_names {
            args.push("/IM");
            // taskkill needs .exe extension
            if name.to_lowercase().ends_with(".exe") {
                args.push(name);
            } else {
                // We need to allocate here, but only once per unique name
                // For static slices, this is acceptable
                let exe_name = Box::leak(format!("{}.exe", name).into_boxed_str());
                args.push(exe_name);
            }
        }
        
        // Fire twice for reliability (matching C# behavior)
        let _ = Command::new("taskkill")
            .args(&args)
            .creation_flags(0x08000000)
            .spawn();
        
        let _ = Command::new("taskkill")
            .args(&args)
            .creation_flags(0x08000000)
            .spawn();
    }

    /// Kill a single process
    #[inline]
    pub fn kill_process(name: &str) {
        let exe_name = if name.to_lowercase().ends_with(".exe") {
            name.to_string()
        } else {
            format!("{}.exe", name)
        };
        
        // Fire twice for reliability
        let _ = Command::new("taskkill")
            .args(["/F", "/IM", &exe_name])
            .creation_flags(0x08000000)
            .spawn();
        
        let _ = Command::new("taskkill")
            .args(["/F", "/IM", &exe_name])
            .creation_flags(0x08000000)
            .spawn();
    }

    /// Restart explorer.exe - 1:1 with C# RestartExplorer()
    /// Only starts explorer if it's NOT already running
    #[inline]
    pub fn restart_explorer() {
        // 1:1 with C#: Check if explorer is already running
        let explorer_running = unsafe {
            let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else { 
                return; 
            };
            if snapshot.is_invalid() { return; }

            let mut entry = PROCESSENTRY32 {
                dwSize: std::mem::size_of::<PROCESSENTRY32>() as u32,
                ..Default::default()
            };

            let mut found = false;
            if Process32First(snapshot, &mut entry).is_ok() {
                loop {
                    let name = Self::extract_process_name(&entry.szExeFile);
                    if name.eq_ignore_ascii_case("explorer") {
                        found = true;
                        break;
                    }
                    if Process32Next(snapshot, &mut entry).is_err() { break; }
                }
            }
            let _ = CloseHandle(snapshot);
            found
        };

        // C#: if (!flag) { Process.Start("explorer.exe"); }
        if !explorer_running {
            let _ = Command::new("explorer.exe").spawn();
        }
    }

    /// Extract process name from PROCESSENTRY32 szExeFile efficiently
    /// Returns name without .exe extension
    #[inline]
    fn extract_process_name(sz_exe_file: &[i8; 260]) -> &str {
        // Find null terminator
        let len = sz_exe_file.iter()
            .position(|&c| c == 0)
            .unwrap_or(260);
        
        // Safe because Windows process names are ASCII
        let bytes = unsafe {
            std::slice::from_raw_parts(sz_exe_file.as_ptr() as *const u8, len)
        };
        
        let name = std::str::from_utf8(bytes).unwrap_or("");
        
        // Remove .exe extension
        name.strip_suffix(".exe")
            .or_else(|| name.strip_suffix(".EXE"))
            .unwrap_or(name)
    }
}
