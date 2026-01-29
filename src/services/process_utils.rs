//! ProcessUtils - 1:1 port of ProcessUtils.cs
//! Note: This functionality is also available in process.rs which is used by GameModeService.
//! Kept for API parity with C# codebase.

#![allow(dead_code)]

use windows::Win32::Foundation::{HANDLE, CloseHandle};
use windows::Win32::System::Threading::{OpenProcess, PROCESS_SUSPEND_RESUME};

// C# ProcessUtils uses P/Invoke on ntdll.dll
#[link(name = "ntdll")]
extern "system" {
    fn NtSuspendProcess(process_handle: HANDLE) -> i32;
    fn NtResumeProcess(process_handle: HANDLE) -> i32;
}

pub struct ProcessUtils;

impl ProcessUtils {
    // 1:1 with public static void SuspendProcess(int pid)
    pub fn suspend_process(pid: u32) {
        unsafe {
            if let Ok(handle) = OpenProcess(PROCESS_SUSPEND_RESUME, false, pid) {
                NtSuspendProcess(handle);
                let _ = CloseHandle(handle);
            }
        }
    }

    // 1:1 with public static void ResumeProcess(int pid)
    pub fn resume_process(pid: u32) {
        unsafe {
            if let Ok(handle) = OpenProcess(PROCESS_SUSPEND_RESUME, false, pid) {
                NtResumeProcess(handle);
                let _ = CloseHandle(handle);
            }
        }
    }
}
