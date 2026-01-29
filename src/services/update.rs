use serde::Deserialize;
use std::env;
use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::process::Command;
use std::thread;

#[derive(Deserialize, Debug)]
pub struct GitHubRelease {
    pub tag_name: String,
    pub assets: Vec<GitHubAsset>,
}

#[derive(Deserialize, Debug)]
pub struct GitHubAsset {
    pub name: String,
    pub browser_download_url: String,
}

pub struct UpdateService;

impl UpdateService {
    // 1:1 CheckForUpdatesAsync logic (Synchronous wrapper for thread usage)
    pub fn check_for_updates() {
        thread::spawn(move || {
            if let Ok(release) = Self::get_latest_release() {
                // Version parsing logic from C#
                // C# compares TagName with "1.0.0"
                // Assuming current version is 1.0.0
                let current_version = "1.0.0";
                let tag = release.tag_name.trim_start_matches('v');
                
                // Simple string compare or semver? C# used Version.TryParse
                // We'll simplisticly assume if tag != current, it's new for this MVP port
                if tag != current_version {
                    // Logic: Show Native MessageBox "Update Available"
                    // In Slint, showing a message box from background thread is hard without callback.
                    // But C# uses `ModernMessageBox.ShowDialog()`.
                    // We can print or use a Win32 MessageBox.
                    
                    use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_YESNO, MB_ICONQUESTION, IDYES};
                    use windows::core::HSTRING;
                    
                    unsafe {
                        let msg = format!("A new version ({}) is available!\n\nDo you want to update now?", release.tag_name);
                        let title = "Update Available";
                        
                        let result = MessageBoxW(None, &HSTRING::from(msg), &HSTRING::from(title), MB_YESNO | MB_ICONQUESTION);
                        if result == IDYES {
                             Self::perform_update(&release);
                        }
                    }
                }
            }
        });
    }

    fn get_latest_release() -> Result<GitHubRelease, Box<dyn std::error::Error>> {
        let url = "https://api.github.com/repos/xillyservices-code/GameMode/releases/latest";
        let agent = ureq::AgentBuilder::new().user_agent("XillyGameMode-Updater").build();
        let resp = agent.get(url).call()?;
        let release: GitHubRelease = resp.into_json()?;
        Ok(release)
    }

    fn perform_update(release: &GitHubRelease) {
         // Find exe asset
         if let Some(asset) = release.assets.iter().find(|a| a.name.ends_with(".exe")) {
             let url = &asset.browser_download_url;
             if let Ok(bytes) = Self::download_file(url) {
                  let current_exe = env::current_exe().unwrap_or(PathBuf::from("gamemode.exe"));
                  let update_exe = current_exe.with_extension("update");
                  
                  if fs::write(&update_exe, bytes).is_ok() {
                      // Create bat file
                      let bat_file = env::temp_dir().join("gamemode_update.bat");
                      let pid = std::process::id();
                      let current_exe_str = current_exe.to_string_lossy();
                      let update_exe_str = update_exe.to_string_lossy();
                      
                      // 1:1 Batch file content logic
                      let script = format!(
                          "@echo off\r\ntimeout /t 2 /nobreak\r\n:loop\r\ntasklist | find \"{}\" >nul\r\nif not errorlevel 1 (\r\n    timeout /t 1 /nobreak\r\n    goto loop\r\n)\r\nif exist \"{}.bak\" del \"{}.bak\"\r\nmove \"{}\" \"{}.bak\"\r\nmove \"{}\" \"{}\"\r\nstart \"\" \"{}\"\r\ndel \"%~f0\"\r\n",
                          pid, current_exe_str, current_exe_str, current_exe_str, current_exe_str, update_exe_str, current_exe_str, current_exe_str
                      );
                      
                      if fs::write(&bat_file, script).is_ok() {
                          let _ = Command::new("cmd")
                              .args(["/C", bat_file.to_str().unwrap()])
                              .spawn();
                          std::process::exit(0);
                      }
                  }
             }
         }
    }

    fn download_file(url: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let agent = ureq::AgentBuilder::new().user_agent("XillyGameMode-Updater").build();
        let resp = agent.get(url).call()?;
        let mut reader = resp.into_reader();
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes)?;
        Ok(bytes)
    }
}
