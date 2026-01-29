use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowRect, GetSystemMetrics, SetForegroundWindow,
    EnumWindows, IsWindowVisible, SM_CXSCREEN, SM_CYSCREEN,
    GetWindowThreadProcessId,
};
use windows::Win32::Foundation::{HWND, RECT, BOOL, LPARAM, CloseHandle};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32First, Process32Next, PROCESSENTRY32, TH32CS_SNAPPROCESS
};
use std::process::Command;
use std::os::windows::process::CommandExt;
use std::sync::atomic::{AtomicU32, AtomicPtr, Ordering};

pub struct GameDetector;

// Static arrays for known games (zero allocation)
static KNOWN_GAMES: &[&str] = &[
    "cod", "cod24-cod", "FortniteClient-Win64-Shipping", "r5apex", "cs2", 
    "valheim", "dota2", "League of Legends", "Overwatch", "Valorant-Win64-Shipping",
    "GTA5", "RDR2", "Cyberpunk2077", "Minecraft.Windows"
];

static EXCLUDED_PROCESSES: &[&str] = &[
    "explorer", "SearchApp", "LockApp", "SearchHost"
];

// Desktop chassis types (static)
static DESKTOP_CHASSIS: &[&str] = &["3", "4", "6", "7", "13", "35"];

impl GameDetector {
    /// Detect fullscreen game - Optimized single-pass version
    /// Returns Option<(pid, hwnd)>
    pub fn detect_fullscreen_game() -> Option<(u32, HWND)> {
        let current_pid = std::process::id();
        let screen_w = unsafe { GetSystemMetrics(SM_CXSCREEN) };
        let screen_h = unsafe { GetSystemMetrics(SM_CYSCREEN) };
        
        unsafe {
            let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else { 
                return None; 
            };
            if snapshot.is_invalid() { return None; }

            let mut entry = PROCESSENTRY32 {
                dwSize: std::mem::size_of::<PROCESSENTRY32>() as u32,
                ..Default::default()
            };

            let mut result = None;

            if Process32First(snapshot, &mut entry).is_ok() {
                'outer: loop {
                    let pid = entry.th32ProcessID;
                    
                    // Skip self
                    if pid == current_pid {
                        if Process32Next(snapshot, &mut entry).is_err() { break; }
                        continue;
                    }

                    // Extract name efficiently
                    let name = Self::extract_name(&entry.szExeFile);
                    
                    // Skip excluded processes
                    if EXCLUDED_PROCESSES.iter().any(|&e| e.eq_ignore_ascii_case(name)) {
                        if Process32Next(snapshot, &mut entry).is_err() { break; }
                        continue;
                    }
                    
                    // Check if known game (priority)
                    let is_known_game = KNOWN_GAMES.iter().any(|&g| g.eq_ignore_ascii_case(name));
                    
                    // Get main window for this process
                    if let Some(hwnd) = Self::get_main_window(pid) {
                        if is_known_game {
                            // Known game found with visible window
                            result = Some((pid, hwnd));
                            break 'outer;
                        }
                        
                        // Check if fullscreen
                        let mut rect = RECT::default();
                        if GetWindowRect(hwnd, &mut rect).is_ok() {
                            let width = rect.right - rect.left;
                            let height = rect.bottom - rect.top;
                            
                            // C# uses >= for fullscreen detection
                            if width >= screen_w && height >= screen_h {
                                result = Some((pid, hwnd));
                                break 'outer;
                            }
                        }
                    }

                    if Process32Next(snapshot, &mut entry).is_err() { break; }
                }
            }
            
            let _ = CloseHandle(snapshot);
            result
        }
    }

    /// Get main window for a process - Optimized
    fn get_main_window(pid: u32) -> Option<HWND> {
        static TARGET_PID: AtomicU32 = AtomicU32::new(0);
        static FOUND_HWND: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(std::ptr::null_mut());
        
        TARGET_PID.store(pid, Ordering::SeqCst);
        FOUND_HWND.store(std::ptr::null_mut(), Ordering::SeqCst);
        
        unsafe extern "system" fn callback(hwnd: HWND, _: LPARAM) -> BOOL {
            let mut window_pid: u32 = 0;
            GetWindowThreadProcessId(hwnd, Some(&mut window_pid));
            
            if window_pid == TARGET_PID.load(Ordering::SeqCst) && IsWindowVisible(hwnd).as_bool() {
                FOUND_HWND.store(hwnd.0, Ordering::SeqCst);
                return BOOL(0); // Stop enumeration
            }
            BOOL(1)
        }
        
        unsafe {
            let _ = EnumWindows(Some(callback), LPARAM(0));
            let found = FOUND_HWND.load(Ordering::SeqCst);
            if !found.is_null() {
                Some(HWND(found))
            } else {
                None
            }
        }
    }

    /// Focus window
    #[inline]
    pub fn focus_window(hwnd: HWND) {
        unsafe {
            let _ = SetForegroundWindow(hwnd);
        }
    }

    /// Check if system is desktop - Cached result
    pub fn is_desktop() -> bool {
        use std::sync::OnceLock;
        static IS_DESKTOP: OnceLock<bool> = OnceLock::new();
        
        *IS_DESKTOP.get_or_init(|| {
            let output = Command::new("wmic")
                .args(["path", "Win32_SystemEnclosure", "get", "ChassisTypes"])
                .creation_flags(0x08000000)
                .output();
            
            if let Ok(o) = output {
                let s = String::from_utf8_lossy(&o.stdout);
                DESKTOP_CHASSIS.iter().any(|&dt| s.split_whitespace().any(|p| p == dt))
            } else {
                false
            }
        })
    }

    /// Extract process name efficiently (no allocation)
    #[inline]
    fn extract_name(sz_exe_file: &[i8; 260]) -> &str {
        let len = sz_exe_file.iter().position(|&c| c == 0).unwrap_or(260);
        let bytes = unsafe { std::slice::from_raw_parts(sz_exe_file.as_ptr() as *const u8, len) };
        let name = std::str::from_utf8(bytes).unwrap_or("");
        name.strip_suffix(".exe").or_else(|| name.strip_suffix(".EXE")).unwrap_or(name)
    }
}
