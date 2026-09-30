use crate::error::Result;
use crate::fsutil;
use crate::models::SteamAccount;
use crate::valve_keyvalues::ValveKeyValues;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub const STEAM_ID_BASE: u64 = 76_561_197_960_265_728;
const CONFIG_RELATIVE_PATH: &str = "730/local/cfg/cs2_video.txt";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginUser {
    pub steam_id64: u64,
    pub account_name: Option<String>,
    pub persona_name: String,
    pub most_recent: bool,
}

#[derive(Clone, Copy, Default)]
pub struct SteamService;

impl SteamService {
    pub fn to_steam_id64(account_id: u32) -> u64 {
        STEAM_ID_BASE + account_id as u64
    }

    pub fn get_roots(&self, override_root: Option<&str>) -> Vec<String> {
        let mut candidates: Vec<String> = Vec::new();
        if let Some(root) = override_root {
            if !root.trim().is_empty() {
                candidates.push(root.to_string());
            }
        }
        candidates.extend(self.registry_roots());
        if let Some(program_files) = program_files_x86() {
            candidates.push(
                Path::new(&program_files)
                    .join("Steam")
                    .to_string_lossy()
                    .to_string(),
            );
        }
        // Deduplicate case-insensitively, keep existing directories only.
        let mut seen = std::collections::HashSet::new();
        let mut roots = Vec::new();
        for candidate in candidates {
            let path = Path::new(&candidate);
            if !path.is_dir() {
                continue;
            }
            let full = fsutil::absolute_lossy(path).to_string_lossy().to_string();
            let key = fsutil::path_key(Path::new(&full));
            if seen.insert(key) {
                roots.push(full);
            }
        }
        roots
    }

    pub fn get_accounts(&self, roots: &[String]) -> Vec<SteamAccount> {
        self.get_accounts_with_warnings(roots).0
    }

    pub fn get_accounts_with_warnings(&self, roots: &[String]) -> (Vec<SteamAccount>, Vec<String>) {
        let mut accounts: HashMap<String, SteamAccount> = HashMap::new();
        let mut warnings = Vec::new();
        for root in roots {
            let root_path = Path::new(root);
            let users =
                match self.parse_login_users(&root_path.join("config").join("loginusers.vdf")) {
                    Ok(users) => users,
                    Err(_) => {
                        warnings.push(format!(
                            "Could not read Steam account names from {}",
                            root_path.display()
                        ));
                        Vec::new()
                    }
                }
                .into_iter()
                .map(|u| (u.steam_id64, u))
                .collect::<HashMap<_, _>>();
            let userdata = root_path.join("userdata");
            if !userdata.is_dir() {
                continue;
            }
            let Ok(directories) = fs::read_dir(&userdata) else {
                continue;
            };
            for directory in directories.flatten() {
                // EnumerateDirectories in .NET yields directories only; a
                // stray file named "12345" must not become a phantom account.
                if !directory.path().is_dir() {
                    continue;
                }
                let name = directory.file_name().to_string_lossy().to_string();
                let Ok(account_id) = name.parse::<u32>() else {
                    continue;
                };
                let steam_id = Self::to_steam_id64(account_id);
                let login = users.get(&steam_id);
                let config = directory.path().join(CONFIG_RELATIVE_PATH);
                let config_str = config.to_string_lossy().to_string();
                let exists = config.is_file();
                let persona = login
                    .map(|l| l.persona_name.clone())
                    .unwrap_or_else(|| format!("Steam account {account_id}"));
                let mut display =
                    format!("{persona}  -  Account ID {account_id}  -  SteamID64 {steam_id}");
                if !exists {
                    display.push_str("  (CS2 config not found)");
                }
                let last_write = if exists {
                    fsutil::file_mtime_nanos(&config)
                } else {
                    i64::MIN
                };
                accounts.insert(
                    fsutil::path_key(Path::new(&config_str)),
                    SteamAccount {
                        display_name: display,
                        account_id: account_id.to_string(),
                        steam_id64: steam_id,
                        persona_name: persona,
                        account_name: login.and_then(|l| l.account_name.clone()),
                        most_recent: login.map(|l| l.most_recent).unwrap_or(false),
                        config_path: config_str,
                        has_config: exists,
                        last_write_time: last_write,
                    },
                );
            }
        }
        let mut values: Vec<SteamAccount> = accounts.into_values().collect();
        values.sort_by(|a, b| {
            b.most_recent
                .cmp(&a.most_recent)
                .then(b.last_write_time.cmp(&a.last_write_time))
                .then(a.persona_name.cmp(&b.persona_name))
        });
        (values, warnings)
    }

    pub fn parse_login_users(&self, path: &Path) -> Result<Vec<LoginUser>> {
        if !path.is_file() {
            return Ok(Vec::new());
        }
        let text = fs::read_to_string(path)?;
        // File.ReadAllText in .NET strips a UTF-8 BOM before parsing.
        let text = text.strip_prefix('\u{FEFF}').unwrap_or(&text);
        let root = ValveKeyValues::parse(text)?;
        let Some(users) = root.get_objects("users").into_iter().next() else {
            return Ok(Vec::new());
        };
        let mut grouped: HashMap<u64, LoginUser> = HashMap::new();
        for entry in &users.entries {
            let Some(object) = &entry.object else {
                continue;
            };
            let Ok(steam_id) = entry.name.parse::<u64>() else {
                continue;
            };
            grouped.insert(
                steam_id,
                LoginUser {
                    steam_id64: steam_id,
                    account_name: object.get_string("AccountName").map(str::to_string),
                    persona_name: object
                        .get_string("PersonaName")
                        .map(str::to_string)
                        .unwrap_or_else(|| "Unknown Steam account".to_string()),
                    most_recent: object.get_string("MostRecent") == Some("1"),
                },
            );
        }
        let mut users: Vec<LoginUser> = grouped.into_values().collect();
        // Deterministic output order (the C# GroupBy preserves group order).
        users.sort_by_key(|user| user.steam_id64);
        Ok(users)
    }

    #[cfg(windows)]
    fn registry_roots(&self) -> Vec<String> {
        use windows_registry::{CURRENT_USER, LOCAL_MACHINE};
        let mut roots = Vec::new();
        for (hive, subkey) in [
            ("HKCU", r"Software\Valve\Steam"),
            ("HKLM", r"SOFTWARE\WOW6432Node\Valve\Steam"),
            ("HKLM", r"SOFTWARE\Valve\Steam"),
        ] {
            let key = if hive == "HKCU" {
                CURRENT_USER.open(subkey)
            } else {
                LOCAL_MACHINE.open(subkey)
            };
            if let Ok(key) = key {
                if let Ok(value) = key.get_string("SteamPath") {
                    if !value.trim().is_empty() {
                        roots.push(value);
                        continue;
                    }
                }
                if let Ok(value) = key.get_string("InstallPath") {
                    if !value.trim().is_empty() {
                        roots.push(value);
                    }
                }
            }
        }
        roots
    }

    #[cfg(not(windows))]
    fn registry_roots(&self) -> Vec<String> {
        Vec::new()
    }
}

fn program_files_x86() -> Option<String> {
    if let Ok(value) = std::env::var("ProgramFiles(x86)") {
        return Some(value);
    }
    if let Ok(value) = std::env::var("ProgramW6432") {
        return Some(value);
    }
    #[cfg(windows)]
    return Some(r"C:\Program Files (x86)".to_string());
    #[cfg(not(windows))]
    return None;
}
