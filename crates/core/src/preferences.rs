use crate::fsutil;
use crate::models::Preferences;
use crate::video_config::VideoConfigService;
use serde_json::{Map, Value};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

pub struct PreferencesService {
    configs: VideoConfigService,
}

impl PreferencesService {
    pub fn new(configs: VideoConfigService) -> Self {
        Self { configs }
    }

    pub fn default_path() -> PathBuf {
        #[cfg(windows)]
        {
            if let Ok(local) = std::env::var("LOCALAPPDATA") {
                return Path::new(&local).join(Self::versioned_suffix());
            }
        }
        if let Ok(local) = std::env::var("CS2_RESEDIT_SETTINGS_DIR") {
            return Path::new(&local).join("settings.json");
        }
        #[cfg(windows)]
        {
            PathBuf::from("settings.json")
        }
        #[cfg(not(windows))]
        {
            if let Ok(home) = std::env::var("HOME") {
                return Path::new(&home)
                    .join(".local")
                    .join("share")
                    .join(Self::versioned_suffix());
            }
            PathBuf::from("settings.json")
        }
    }

    /// Versioned preferences directory suffix used by tests/docs.
    pub fn versioned_suffix() -> PathBuf {
        Path::new("Softhe")
            .join("CS2-ResEdit")
            .join("v1")
            .join("settings.json")
    }

    pub fn read(&self, path: Option<&Path>) -> Preferences {
        let mut result = Preferences::default();
        let path = path.map(PathBuf::from).unwrap_or_else(Self::default_path);
        if !path.is_file() {
            return result;
        }
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(err) => {
                result.warning = Some(format!("Preferences could not be loaded: {err}"));
                return result;
            }
        };
        // File.ReadAllText in .NET strips a UTF-8 BOM; serde_json does not.
        let text = text.strip_prefix('\u{FEFF}').unwrap_or(&text);
        let parsed: Value = match serde_json::from_str(text) {
            Ok(value) => value,
            Err(err) => {
                result.warning = Some(format!("Preferences could not be loaded: {err}"));
                return result;
            }
        };
        let Some(object) = parsed.as_object() else {
            result.warning = if parsed.is_null() {
                Some("Preferences could not be loaded: Settings are empty.".to_string())
            } else {
                Some("Preferences could not be loaded: Invalid settings JSON.".to_string())
            };
            return result;
        };
        let invalid = || "Preferences could not be loaded: Invalid settings JSON.".to_string();
        // A missing SchemaVersion keeps the C# initializer default of 1.
        let schema = match field(object, "SchemaVersion") {
            None => 1,
            Some(Value::Null) | Some(Value::Bool(_)) => {
                result.warning = Some(invalid());
                return result;
            }
            Some(value) => match value.as_i64() {
                Some(schema) => schema,
                None => {
                    result.warning = Some(invalid());
                    return result;
                }
            },
        };
        if schema != 1 {
            result.warning = Some(format!(
                "Preferences could not be loaded: Unsupported preference schema '{schema}'."
            ));
            return result;
        }
        let last_account_id = match field(object, "LastAccountId") {
            None | Some(Value::Null) => None,
            Some(Value::String(text)) => Some(text.clone()),
            Some(_) => {
                result.warning = Some(invalid());
                return result;
            }
        };
        let last_config_path = match field(object, "LastConfigPath") {
            None | Some(Value::Null) => None,
            Some(Value::String(text)) => Some(text.clone()),
            Some(_) => {
                result.warning = Some(invalid());
                return result;
            }
        };
        let recent_raw: Vec<String> = match field(object, "RecentConfigPaths") {
            None | Some(Value::Null) => Vec::new(),
            Some(Value::Array(items)) => {
                let mut out = Vec::new();
                for item in items {
                    match item {
                        Value::String(text) => out.push(text.clone()),
                        // Null entries are preserved as null in the C# list
                        // and skipped by Clean.
                        Value::Null => {}
                        _ => {
                            result.warning = Some(invalid());
                            return result;
                        }
                    }
                }
                out
            }
            Some(_) => {
                result.warning = Some(invalid());
                return result;
            }
        };
        result.last_account_id = last_account_id;
        result.theme = field(object, "Theme")
            .and_then(|value| serde_json::from_value(value.clone()).ok())
            .unwrap_or_default();
        result.last_config_path = last_config_path
            .filter(|candidate| self.configs.read(Path::new(candidate)).is_ok())
            .map(|candidate| normalize(&candidate).0);
        result.recent_config_paths = self.clean(&recent_raw);
        result
    }

    pub fn save(
        &self,
        last_account_id: Option<&str>,
        recent_paths: &[String],
        path: Option<&Path>,
    ) -> Preferences {
        self.save_selection(last_account_id, None, recent_paths, path)
    }

    pub fn save_selection(
        &self,
        last_account_id: Option<&str>,
        last_config_path: Option<&Path>,
        recent_paths: &[String],
        path: Option<&Path>,
    ) -> Preferences {
        let theme = self.read(path).theme;
        self.save_selection_with_theme(last_account_id, last_config_path, recent_paths, theme, path)
    }

    pub fn save_selection_with_theme(
        &self,
        last_account_id: Option<&str>,
        last_config_path: Option<&Path>,
        recent_paths: &[String],
        theme: crate::models::UiTheme,
        path: Option<&Path>,
    ) -> Preferences {
        let mut result = Preferences {
            theme,
            schema_version: 1,
            last_account_id: last_account_id
                .filter(|s| !s.trim().is_empty())
                .map(str::to_string),
            last_config_path: last_config_path
                .map(|path| fsutil::absolute_lossy(path).to_string_lossy().to_string()),
            recent_config_paths: self.clean(recent_paths),
            warning: None,
        };
        let path = path.map(PathBuf::from).unwrap_or_else(Self::default_path);
        if let Some(parent) = fsutil::absolute_lossy(&path)
            .parent()
            .map(Path::to_path_buf)
        {
            if let Err(err) = fs::create_dir_all(&parent) {
                result.warning = Some(format!("Preferences could not be saved: {err}"));
                return result;
            }
        }
        let json = serde_json::json!({
            "SchemaVersion": result.schema_version,
            "Theme": result.theme,
            "LastAccountId": result.last_account_id,
            "LastConfigPath": result.last_config_path,
            "RecentConfigPaths": result.recent_config_paths,
        });
        let text = serde_json::to_string_pretty(&json).expect("preferences serialize");
        // Atomic replacement keeps settings.json valid even under crashes.
        if let Err(err) = fsutil::atomic_write(&path, text.as_bytes()) {
            result.warning = Some(format!("Preferences could not be saved: {err}"));
        }
        result
    }

    pub fn add_recent(&self, paths: &[String], path: &str) -> Vec<String> {
        let mut ordered: Vec<String> = Vec::new();
        ordered.push(path.to_string());
        ordered.extend(paths.iter().cloned());
        let mut seen = HashSet::new();
        let mut result = Vec::new();
        for candidate in ordered {
            let (full, key) = normalize(&candidate);
            if seen.insert(key) {
                result.push(full);
            }
            if result.len() == 5 {
                break;
            }
        }
        result
    }

    fn clean(&self, paths: &[String]) -> Vec<String> {
        let mut result = Vec::new();
        let mut seen = HashSet::new();
        for candidate in paths {
            if candidate.trim().is_empty() {
                continue;
            }
            let (full, key) = normalize(candidate);
            if seen.contains(&key) || !Path::new(&full).is_file() {
                continue;
            }
            if self.configs.read(Path::new(&full)).is_err() {
                continue;
            }
            seen.insert(key);
            result.push(full);
            if result.len() == 5 {
                break;
            }
        }
        result
    }
}

/// Absolute path plus its case-insensitive dedup key.
fn normalize(candidate: &str) -> (String, String) {
    let full = fsutil::absolute_lossy(Path::new(candidate))
        .to_string_lossy()
        .to_string();
    let key = fsutil::path_key(Path::new(&full));
    (full, key)
}

/// Case-insensitive property lookup, matching JsonSerializerOptions with
/// PropertyNameCaseInsensitive = true.
fn field<'v>(object: &'v Map<String, Value>, name: &str) -> Option<&'v Value> {
    object
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value)
}
