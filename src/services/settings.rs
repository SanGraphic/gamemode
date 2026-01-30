use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::fs;

/// AppSettings - 1:1 port of AppSettings.cs
/// Note: C# has SuspendExplorer (default false), SuspendBrowsers (default true), SuspendLaunchers (default true)
/// IsolateNetwork is in GameModeOptions but we store it in settings for persistence
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppSettings {
    /// Whether to kill explorer.exe during game mode (default: false)
    /// C#: public bool SuspendExplorer { get; set; }
    #[serde(default)]
    pub suspend_explorer: bool,
    
    /// Whether to kill browser processes during game mode (default: true)
    /// C#: public bool SuspendBrowsers { get; set; } = true;
    #[serde(default = "default_true")]
    pub suspend_browsers: bool,
    
    /// Whether to kill game launcher processes during game mode (default: true)
    /// C#: public bool SuspendLaunchers { get; set; } = true;
    #[serde(default = "default_true")]
    pub suspend_launchers: bool,
    
    /// Whether to enable network isolation (DNS multicast disable, NetBIOS disable)
    /// C#: This is passed via GameModeOptions.IsolateNetwork
    #[serde(default)]
    pub isolate_network: bool,
    
    /// Whether to apply advanced ReviOS-style system tweaks
    /// Includes: service disabling, VBS off, telemetry off, multimedia optimizations
    #[serde(default)]
    pub advanced_tweaks: bool,
    
    /// Whether to disable MPO (Multi-Plane Overlay)
    /// When false: MPO ON + OverlayMinFPS=0
    /// When true: MPO OFF (OverlayTestMode=5)
    #[serde(default)]
    pub disable_mpo: bool,
    
    /// Whether to run on Windows startup
    /// Note: This was not in C# AppSettings but is useful for the app
    #[serde(default)]
    pub run_on_startup: bool,
    
    /// Advanced module settings for 1% lows optimization
    #[serde(default)]
    pub advanced_modules: AdvancedModuleSettings,
}

/// Advanced module settings for hardware-aware 1% low optimizations
/// These are toggleable and only active when game mode is active
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AdvancedModuleSettings {
    /// Disable core parking to prevent micro-stutter from core wake latency
    /// Best for: 6+ core systems
    #[serde(default)]
    pub disable_core_parking: bool,
    
    /// Enable large system pages for better TLB efficiency
    /// Best for: 16GB+ RAM systems
    #[serde(default)]
    pub enable_large_pages: bool,
    
    /// Boost MMCSS (Multimedia Class Scheduler Service) priority for game threads
    /// Reduces scheduling latency for multimedia/game threads
    #[serde(default)]
    pub mmcss_priority_boost: bool,
    
    /// Enable Hardware-Accelerated GPU Scheduling
    /// Best for: RTX 30/40 series, RX 6000/7000 series (2020+ GPUs)
    #[serde(default)]
    pub enable_hags: bool,
    
    /// Demote non-game processes to idle priority during game mode
    /// Reduces CPU contention from background processes
    #[serde(default)]
    pub process_idle_demotion: bool,
    
    /// Lower bufferbloat by disabling TCP autotuning
    /// Reduces network latency spikes during gaming (default: true)
    #[serde(default = "default_true")]
    pub lower_bufferbloat: bool,
}

impl Default for AdvancedModuleSettings {
    fn default() -> Self {
        Self {
            disable_core_parking: false,
            enable_large_pages: false,
            mmcss_priority_boost: false,
            enable_hags: false,
            process_idle_demotion: false,
            lower_bufferbloat: true, // ON by default
        }
    }
}

fn default_true() -> bool { true }

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            suspend_explorer: false,
            suspend_browsers: true,
            suspend_launchers: true,
            isolate_network: false,
            advanced_tweaks: false,
            disable_mpo: false,
            run_on_startup: false,
            advanced_modules: AdvancedModuleSettings::default(),
        }
    }
}

/// SettingsService - 1:1 port of SettingsService.cs
/// Handles loading and saving settings to JSON file in %LOCALAPPDATA%\XillyGameMode
pub struct SettingsService {
    file_path: PathBuf,
}

impl SettingsService {
    /// 1:1 with C# constructor
    /// Creates settings folder in %LOCALAPPDATA%\XillyGameMode if it doesn't exist
    pub fn new() -> Self {
        let app_data = dirs::data_local_dir().unwrap_or(PathBuf::from("."));
        let folder = app_data.join("XillyGameMode");
        if !folder.exists() {
            let _ = fs::create_dir_all(&folder);
        }
        Self {
            file_path: folder.join("settings.json"),
        }
    }

    /// 1:1 with C# LoadSettingsAsync (synchronous version)
    pub fn load(&self) -> AppSettings {
        if self.file_path.exists() {
            if let Ok(content) = fs::read_to_string(&self.file_path) {
                if let Ok(settings) = serde_json::from_str(&content) {
                    return settings;
                }
            }
        }
        AppSettings::default()
    }

    /// 1:1 with C# SaveSettingsAsync (synchronous version)
    pub fn save(&self, settings: &AppSettings) {
        if let Ok(content) = serde_json::to_string_pretty(settings) {
            let _ = fs::write(&self.file_path, content);
        }
    }
}
