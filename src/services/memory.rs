use windows::Win32::System::ProcessStatus::EmptyWorkingSet;
use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SET_QUOTA};
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32First, Process32Next, PROCESSENTRY32, TH32CS_SNAPPROCESS
};

pub struct MemoryService;

impl MemoryService {
    /// 1:1 FlushMemoryAsync - Optimized version
    /// Empties working set of all processes except self
    #[inline]
    pub fn flush_memory() {
        let self_pid = std::process::id();
        
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
                    
                    // Skip self (1:1 with C#: process.Id != currentProcess.Id)
                    if pid != self_pid {
                        // C# checks process.Handle != IntPtr.Zero
                        // OpenProcess returns error if we can't access
                        if let Ok(handle) = OpenProcess(
                            PROCESS_SET_QUOTA | PROCESS_QUERY_LIMITED_INFORMATION, 
                            false, 
                            pid
                        ) {
                            // EmptyWorkingSet - same as C# psapi.dll call
                            let _ = EmptyWorkingSet(handle);
                            let _ = CloseHandle(handle);
                        }
                    }

                    if Process32Next(snapshot, &mut entry).is_err() { break; }
                }
            }
            
            let _ = CloseHandle(snapshot);
        }
    }
}
