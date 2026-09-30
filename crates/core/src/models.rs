use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct Resolution {
    pub width: i32,
    pub height: i32,
    pub ratio: &'static str,
    pub mode: i32,
}

impl Resolution {
    pub const fn new(width: i32, height: i32, ratio: &'static str, mode: i32) -> Self {
        Self {
            width,
            height,
            ratio,
            mode,
        }
    }

    pub fn display(&self) -> String {
        format!("{}x{}", self.width, self.height)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VideoConfigState {
    pub width: i32,
    pub height: i32,
    pub aspect_mode: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteamAccount {
    pub display_name: String,
    pub account_id: String,
    pub steam_id64: u64,
    pub persona_name: String,
    pub account_name: Option<String>,
    pub most_recent: bool,
    pub config_path: String,
    pub has_config: bool,
    /// Nanoseconds since UNIX epoch; i64::MIN when no config exists.
    pub last_write_time: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupInfo {
    pub path: String,
    pub created: i64,
    pub state: VideoConfigState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Preferences {
    #[serde(default)]
    pub theme: UiTheme,
    #[serde(default = "default_schema_version")]
    pub schema_version: i32,
    #[serde(default)]
    pub last_account_id: Option<String>,
    #[serde(default)]
    pub last_config_path: Option<String>,
    #[serde(default)]
    pub recent_config_paths: Vec<String>,
    #[serde(default, skip_serializing)]
    pub warning: Option<String>,
}

fn default_schema_version() -> i32 {
    1
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            schema_version: 1,
            theme: UiTheme::default(),
            last_account_id: None,
            last_config_path: None,
            recent_config_paths: Vec::new(),
            warning: None,
        }
    }
}

/// Stable preference identifiers for the selectable application themes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum UiTheme {
    #[default]
    Glacier,
    Paper,
    Monolith,
    Canopy,
}

impl UiTheme {
    pub const ALL: [Self; 4] = [Self::Glacier, Self::Paper, Self::Monolith, Self::Canopy];

    pub fn label(self) -> &'static str {
        match self {
            Self::Glacier => "Glacier",
            Self::Paper => "Paper",
            Self::Monolith => "Monolith",
            Self::Canopy => "Canopy",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateResult {
    pub changed: bool,
    pub backup_path: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DisplayResolution {
    pub width: i32,
    pub height: i32,
}

impl DisplayResolution {
    pub const fn new(width: i32, height: i32) -> Self {
        Self { width, height }
    }

    pub fn display(&self) -> String {
        format!("{}x{}", self.width, self.height)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DisplayInfo {
    pub device_name: String,
    pub friendly_name: String,
    pub is_primary: bool,
    pub current_width: i32,
    pub current_height: i32,
    pub modes: Vec<DisplayResolution>,
}

impl DisplayInfo {
    pub fn label(&self) -> String {
        let display = self
            .device_name
            .strip_prefix("\\\\.\\")
            .unwrap_or(&self.device_name);
        format!(
            "{}  ·  {}  ·  {}x{}{}",
            self.friendly_name,
            display,
            self.current_width,
            self.current_height,
            if self.is_primary { "  ·  Primary" } else { "" }
        )
    }
}

impl std::fmt::Display for DisplayInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigurationInspection {
    pub state: VideoConfigState,
    pub encoding: String,
    pub has_bom: bool,
    pub line_ending: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticDisplay {
    pub is_primary: bool,
    pub current_width: i32,
    pub current_height: i32,
    pub mode_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticReport {
    pub schema_version: i32,
    #[serde(rename = "generated_utc")]
    pub generated_utc: String,
    pub application_version: String,
    pub operating_system: String,
    pub architecture: String,
    pub displays: Vec<DiagnosticDisplay>,
    pub steam_root_count: usize,
    pub account_count: usize,
    pub configuration_status: String,
    pub configuration_encoding: Option<String>,
    pub configuration_has_bom: Option<bool>,
    pub configuration_line_ending: Option<String>,
    pub error_category: Option<String>,
}
