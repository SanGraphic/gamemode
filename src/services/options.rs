use serde::{Deserialize, Serialize};

/// GameModeOptions - 1:1 Port of GameModeOptions.cs
/// Options passed to enable/disable game mode
/// 
/// C# Source:
/// ```csharp
/// public class GameModeOptions
/// {
///     public bool SuspendExplorer { get; set; }
///     public bool SuspendBrowsers { get; set; }
///     public bool SuspendLaunchers { get; set; }
///     public bool IsolateNetwork { get; set; }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GameModeOptions {
    /// Whether to kill explorer.exe (C#: SuspendExplorer)
    #[serde(rename = "SuspendExplorer")]
    pub suspend_explorer: bool,

    /// Whether to kill browser processes (C#: SuspendBrowsers)
    #[serde(rename = "SuspendBrowsers")]
    pub suspend_browsers: bool,

    /// Whether to kill game launcher processes (C#: SuspendLaunchers)
    #[serde(rename = "SuspendLaunchers")]
    pub suspend_launchers: bool,
    
    /// Whether to enable network isolation (C#: IsolateNetwork)
    #[serde(rename = "IsolateNetwork")]
    pub isolate_network: bool,
}

impl GameModeOptions {
    /// Create GameModeOptions from AppSettings
    #[allow(dead_code)]
    pub fn from_settings(settings: &crate::services::settings::AppSettings) -> Self {
        Self {
            suspend_explorer: settings.suspend_explorer,
            suspend_browsers: settings.suspend_browsers,
            suspend_launchers: settings.suspend_launchers,
            isolate_network: settings.isolate_network,
        }
    }
}
