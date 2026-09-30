use crate::error::{CoreError, Result};
use crate::fsutil;
use crate::models::{
    BackupInfo, ConfigurationInspection, Resolution, UpdateResult, VideoConfigState,
};
use chrono::Local;
use regex::Regex;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

const WIDTH_KEY: &str = "setting.defaultres";
const HEIGHT_KEY: &str = "setting.defaultresheight";
const MODE_KEY: &str = "setting.aspectratiomode";
static WRITE_LOCK: Mutex<()> = Mutex::new(());

fn value_pattern(key: &str) -> String {
    format!(
        r#"(?im)^(?P<prefix>[ \t]*"{key}"[ \t]+"?)(?P<value>-?\d+)(?P<suffix>"?[ \t]*(?://[^\r\n]*)?\r?)$"#,
        key = regex::escape(key)
    )
}

fn exactly_one_error(key: &str) -> CoreError {
    CoreError::InvalidData(format!(
        "Configuration must contain exactly one '{key}' entry."
    ))
}

fn value_error(key: &str) -> CoreError {
    CoreError::InvalidData(format!("Configuration value '{key}' is invalid."))
}

fn value_regex(key: &str) -> &'static Regex {
    match key {
        WIDTH_KEY => {
            static RE: OnceLock<Regex> = OnceLock::new();
            RE.get_or_init(|| Regex::new(&value_pattern(WIDTH_KEY)).expect("valid value regex"))
        }
        HEIGHT_KEY => {
            static RE: OnceLock<Regex> = OnceLock::new();
            RE.get_or_init(|| Regex::new(&value_pattern(HEIGHT_KEY)).expect("valid value regex"))
        }
        MODE_KEY => {
            static RE: OnceLock<Regex> = OnceLock::new();
            RE.get_or_init(|| Regex::new(&value_pattern(MODE_KEY)).expect("valid value regex"))
        }
        _ => unreachable!("unknown configuration key"),
    }
}

fn balanced_value_quotes(caps: &regex::Captures<'_>) -> bool {
    caps["prefix"].ends_with('"') == caps["suffix"].starts_with('"')
}

/// Matches `prefix + YYYYMMDD-HHMMSS[.bak]` with an optional `-N` collision
/// suffix, case-insensitively like the original `(?i)` regex.
fn is_backup_name(name: &str, prefix: &str) -> bool {
    let lowered = name.to_lowercase();
    let Some(rest) = lowered
        .strip_prefix(&prefix.to_lowercase())
        .and_then(strip_bak_suffix)
    else {
        return false;
    };
    if !rest.is_ascii() {
        return false;
    }
    let (stamp, extra) = match rest.len() {
        15 => (rest, None),
        n if n > 16 => (&rest[..15], Some(&rest[15..])),
        _ => return false,
    };
    let digits = stamp.as_bytes();
    let valid_stamp = digits[..8].iter().all(|c| c.is_ascii_digit())
        && digits[8] == b'-'
        && digits[9..].iter().all(|c| c.is_ascii_digit());
    if !valid_stamp {
        return false;
    }
    match extra {
        None => true,
        Some(suffix) => {
            suffix.len() > 1
                && suffix.as_bytes()[0] == b'-'
                && suffix[1..].bytes().all(|c| c.is_ascii_digit())
        }
    }
}

fn strip_bak_suffix(name: &str) -> Option<&str> {
    let bytes = name.as_bytes();
    if bytes.len() >= 4 && bytes[bytes.len() - 4..].eq_ignore_ascii_case(b".bak") {
        Some(&name[..bytes.len() - 4])
    } else {
        None
    }
}

#[derive(Clone, Copy, Default)]
pub struct VideoConfigService;

/// The exact configuration loaded into the UI, including unrelated settings.
#[derive(Debug, Clone)]
pub struct VideoConfigSnapshot {
    pub state: VideoConfigState,
    bytes: Vec<u8>,
}

impl VideoConfigService {
    pub fn read_snapshot(&self, path: &Path) -> Result<VideoConfigSnapshot> {
        let bytes = fs::read(path)?;
        let document = TextDocument::decode(&bytes)?;
        let state = self.parse(&document.text)?;
        Ok(VideoConfigSnapshot { state, bytes })
    }

    pub fn read(&self, path: &Path) -> Result<VideoConfigState> {
        let document = TextDocument::load(path)?;
        self.parse(&document.text)
    }

    pub fn inspect(&self, path: &Path) -> Result<ConfigurationInspection> {
        let document = TextDocument::load(path)?;
        let state = self.parse(&document.text)?;
        let line_ending = if document.text.contains("\r\n") {
            "CRLF"
        } else if document.text.contains('\n') {
            "LF"
        } else {
            "None"
        };
        Ok(ConfigurationInspection {
            state,
            encoding: document.encoding_name().to_string(),
            has_bom: document.has_bom,
            line_ending: line_ending.to_string(),
        })
    }

    pub fn parse(&self, text: &str) -> Result<VideoConfigState> {
        Ok(VideoConfigState {
            width: Self::read_unique(text, WIDTH_KEY, 1, 32768)?,
            height: Self::read_unique(text, HEIGHT_KEY, 1, 32768)?,
            aspect_mode: Self::read_unique(text, MODE_KEY, 0, 2)?,
        })
    }

    pub fn update(
        &self,
        path: &Path,
        resolution: &Resolution,
        create_backup: bool,
    ) -> Result<UpdateResult> {
        self.update_inner(path, resolution, create_backup, None)
    }

    pub fn update_if_unchanged(
        &self,
        path: &Path,
        resolution: &Resolution,
        create_backup: bool,
        snapshot: &VideoConfigSnapshot,
    ) -> Result<UpdateResult> {
        self.update_inner(path, resolution, create_backup, Some(snapshot))
    }

    fn update_inner(
        &self,
        path: &Path,
        resolution: &Resolution,
        create_backup: bool,
        snapshot: Option<&VideoConfigSnapshot>,
    ) -> Result<UpdateResult> {
        let _write_guard = WRITE_LOCK.lock().unwrap_or_else(|err| err.into_inner());
        let full_path = fsutil::absolute(path)
            .map_err(|e| CoreError::from(e).context("Resolving the configuration path failed"))?;
        let original_bytes = fs::read(&full_path)
            .map_err(|e| CoreError::from(e).context("Reading the configuration failed"))?;
        check_snapshot(snapshot, &original_bytes)?;
        let document = TextDocument::decode(&original_bytes)?;
        let current = self.parse(&document.text)?;
        let desired = VideoConfigState {
            width: resolution.width,
            height: resolution.height,
            aspect_mode: resolution.mode,
        };
        if current == desired {
            return Ok(UpdateResult {
                changed: false,
                backup_path: None,
            });
        }

        let mut changed = Self::replace_unique(&document.text, WIDTH_KEY, resolution.width)?;
        changed = Self::replace_unique(&changed, HEIGHT_KEY, resolution.height)?;
        changed = Self::replace_unique(&changed, MODE_KEY, resolution.mode)?;
        // Validate before writing.
        let _ = self.parse(&changed)?;

        if snapshot.is_some() {
            let latest = fs::read(&full_path)?;
            check_snapshot(snapshot, &latest)?;
        }

        let backup_path = if create_backup {
            Some(
                write_backup(&full_path, &original_bytes)
                    .map_err(|e| e.context("Creating the backup failed"))?,
            )
        } else {
            None
        };

        // Keep the comparison as close to the replacement as practical. The
        // backup is made from the checked bytes, never from a later version.
        if snapshot.is_some() {
            let latest = fs::read(&full_path)?;
            if let Err(err) = check_snapshot(snapshot, &latest) {
                if let Some(backup) = &backup_path {
                    let _ = fs::remove_file(backup);
                }
                return Err(err);
            }
        }

        let changed_bytes = document.encode(&changed);
        let write_result = fsutil::atomic_write(&full_path, &changed_bytes)
            .map_err(|e| CoreError::from(e).context("Writing the configuration failed"));
        match write_result {
            Ok(()) => {
                if create_backup {
                    // Mirror the C# catch block: a pruning failure rolls the
                    // update back instead of reporting success-with-error.
                    if let Err(err) = self.retain_backups(&full_path) {
                        let err = err.context("Pruning old backups failed");
                        if let Some(backup) = &backup_path {
                            restore_if_current_matches(backup, &full_path, &changed_bytes)
                                .map_err(|rollback| {
                                    CoreError::InvalidData(format!(
                                    "{err}; rolling back the configuration also failed: {rollback}"
                                ))
                                })?;
                        }
                        return Err(err);
                    }
                }
                Ok(UpdateResult {
                    changed: true,
                    backup_path: backup_path.map(|p| p.to_string_lossy().to_string()),
                })
            }
            Err(err) => {
                // A failed atomic replacement normally leaves the live file
                // untouched. Writing the backup over it here could erase a
                // concurrent edit made by another process.
                Err(err)
            }
        }
    }

    pub fn get_backups(&self, config_path: &Path) -> Result<Vec<BackupInfo>> {
        let full_path = fsutil::absolute(config_path)?;
        let directory = full_path.parent().ok_or_else(|| {
            CoreError::InvalidData("Configuration path has no parent.".to_string())
        })?;
        if !directory.exists() {
            return Ok(Vec::new());
        }
        let file_name = full_path
            .file_name()
            .ok_or_else(|| {
                CoreError::InvalidData("Configuration path has no file name.".to_string())
            })?
            .to_string_lossy()
            .to_string();
        let prefix = format!("{file_name}.");
        let mut backups = Vec::new();
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            if !is_backup_name(&name, &prefix) {
                continue;
            }
            let created = fsutil::file_mtime_nanos(&path);
            match self.read(&path) {
                Ok(state) => backups.push(BackupInfo {
                    path: path.to_string_lossy().to_string(),
                    created,
                    state,
                }),
                Err(_) => continue,
            }
        }
        backups.sort_by(|a, b| b.created.cmp(&a.created).then(b.path.cmp(&a.path)));
        Ok(backups)
    }

    pub fn restore(&self, config_path: &Path, backup_path: &Path) -> Result<PathBuf> {
        self.restore_inner(config_path, backup_path, None)
    }

    pub fn restore_if_unchanged(
        &self,
        config_path: &Path,
        backup_path: &Path,
        snapshot: &VideoConfigSnapshot,
    ) -> Result<PathBuf> {
        self.restore_inner(config_path, backup_path, Some(snapshot))
    }

    fn restore_inner(
        &self,
        config_path: &Path,
        backup_path: &Path,
        snapshot: Option<&VideoConfigSnapshot>,
    ) -> Result<PathBuf> {
        let _write_guard = WRITE_LOCK.lock().unwrap_or_else(|err| err.into_inner());
        let full_config = fsutil::absolute(config_path)?;
        let full_backup = fsutil::absolute(backup_path)?;
        if snapshot.is_some() {
            check_snapshot(snapshot, &fs::read(&full_config)?)?;
        }
        // Validate both files before touching anything.
        let _ = self.read(&full_config)?;
        let bytes = fs::read(&full_backup)?;
        let backup_document = TextDocument::decode(&bytes)?;
        let _ = self.parse(&backup_document.text)?;
        let recognized = backup_paths(&full_config)?
            .iter()
            .any(|b| paths_equal(b, &full_backup));
        if !recognized {
            return Err(CoreError::NotABackup(
                "The selected file is not a recognized backup for this configuration.".to_string(),
            ));
        }

        if snapshot.is_some() {
            check_snapshot(snapshot, &fs::read(&full_config)?)?;
        }

        let current_bytes = fs::read(&full_config)?;
        let rollback = write_backup(&full_config, &current_bytes)?;
        if snapshot.is_some() {
            let latest = fs::read(&full_config)?;
            if let Err(err) = check_snapshot(snapshot, &latest) {
                let _ = fs::remove_file(&rollback);
                return Err(err);
            }
        }
        match fsutil::atomic_write(&full_config, &bytes) {
            Ok(()) => {
                if let Err(err) = self.retain_backups(&full_config) {
                    restore_if_current_matches(&rollback, &full_config, &bytes).map_err(|rollback_error| {
                        CoreError::InvalidData(format!(
                            "Pruning old backups failed: {err}; rolling back the configuration also failed: {rollback_error}"
                        ))
                    })?;
                    return Err(err.context("Pruning old backups failed"));
                }
                Ok(rollback)
            }
            Err(err) => Err(err.into()),
        }
    }

    fn read_unique(text: &str, key: &str, min: i32, max: i32) -> Result<i32> {
        let re = value_regex(key);
        // Single pass: stop as soon as a second match proves duplication.
        let mut iter = re.captures_iter(text);
        let Some(caps) = iter.next() else {
            return Err(exactly_one_error(key));
        };
        if iter.next().is_some() {
            return Err(exactly_one_error(key));
        }
        if !balanced_value_quotes(&caps) {
            return Err(value_error(key));
        }
        let raw = caps.name("value").map(|m| m.as_str()).unwrap_or_default();
        let value: i32 = raw.parse().map_err(|_| value_error(key))?;
        if value < min || value > max {
            return Err(value_error(key));
        }
        Ok(value)
    }

    fn replace_unique(text: &str, key: &str, value: i32) -> Result<String> {
        let re = value_regex(key);
        // Single pass: locate the sole match, then splice around it instead
        // of re-scanning with Regex::replace.
        let mut iter = re.captures_iter(text);
        let Some(caps) = iter.next() else {
            return Err(exactly_one_error(key));
        };
        if iter.next().is_some() {
            return Err(exactly_one_error(key));
        }
        if !balanced_value_quotes(&caps) {
            return Err(value_error(key));
        }
        let whole = caps.get(0).expect("capture 0 always present");
        let mut out = String::with_capacity(text.len() + 8);
        out.push_str(&text[..whole.start()]);
        out.push_str(&caps["prefix"]);
        out.push_str(&value.to_string());
        out.push_str(&caps["suffix"]);
        out.push_str(&text[whole.end()..]);
        Ok(out)
    }

    fn retain_backups(&self, path: &Path) -> Result<()> {
        for backup in backup_paths(path)?.iter().skip(5) {
            fs::remove_file(backup)?;
        }
        Ok(())
    }
}

fn restore_if_current_matches(backup: &Path, config: &Path, expected: &[u8]) -> Result<()> {
    if fs::read(config)? != expected {
        return Err(CoreError::ExternalChange(
            "The configuration changed again; automatic rollback was skipped to preserve the newer content."
                .to_string(),
        ));
    }
    let bytes = fs::read(backup)?;
    fsutil::atomic_write(config, &bytes)?;
    Ok(())
}

#[cfg(test)]
mod rollback_tests {
    use super::*;

    #[test]
    fn rollback_preserves_a_concurrent_edit() {
        let dir = tempfile::tempdir().expect("tempdir");
        let config = dir.path().join("cs2_video.txt");
        let backup = dir.path().join("cs2_video.txt.backup");
        fs::write(&backup, b"original").expect("backup");
        fs::write(&config, b"concurrent edit").expect("edit");
        assert!(matches!(
            restore_if_current_matches(&backup, &config, b"our write"),
            Err(CoreError::ExternalChange(_))
        ));
        assert_eq!(fs::read(&config).expect("config"), b"concurrent edit");
    }
}

fn check_snapshot(snapshot: Option<&VideoConfigSnapshot>, current: &[u8]) -> Result<()> {
    if snapshot.is_some_and(|loaded| loaded.bytes != current) {
        return Err(CoreError::ExternalChange(
            "The configuration changed outside CS2 ResEdit. Reload it before applying changes."
                .to_string(),
        ));
    }
    Ok(())
}

fn paths_equal(a: &Path, b: &Path) -> bool {
    #[cfg(windows)]
    {
        a.to_string_lossy()
            .eq_ignore_ascii_case(&b.to_string_lossy())
    }
    #[cfg(not(windows))]
    {
        a == b
    }
}

fn write_backup(path: &Path, bytes: &[u8]) -> Result<PathBuf> {
    let stamp = Local::now().format("%Y%m%d-%H%M%S").to_string();
    for index in 0_u32.. {
        let candidate = if index == 0 {
            PathBuf::from(format!("{}.{}.bak", path.to_string_lossy(), stamp))
        } else {
            PathBuf::from(format!(
                "{}.{}-{}.bak",
                path.to_string_lossy(),
                stamp,
                index
            ))
        };
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(mut file) => {
                if let Err(err) = file.write_all(bytes).and_then(|_| file.sync_all()) {
                    drop(file);
                    let _ = fs::remove_file(&candidate);
                    return Err(err.into());
                }
                return Ok(candidate);
            }
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(err.into()),
        }
    }
    unreachable!("the backup attempt counter is unbounded")
}

fn backup_paths(path: &Path) -> Result<Vec<PathBuf>> {
    let directory = path
        .parent()
        .ok_or_else(|| CoreError::InvalidData("Configuration path has no parent.".to_string()))?;
    let file_name = path
        .file_name()
        .ok_or_else(|| CoreError::InvalidData("Configuration path has no file name.".to_string()))?
        .to_string_lossy();
    let prefix = format!("{file_name}.");
    let mut paths = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let candidate = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if candidate.is_file() && is_backup_name(&name, &prefix) {
            paths.push(candidate);
        }
    }
    paths.sort_by(|a, b| {
        fsutil::file_mtime_nanos(b)
            .cmp(&fsutil::file_mtime_nanos(a))
            .then(b.cmp(a))
    });
    Ok(paths)
}

#[derive(Debug, Clone)]
struct TextDocument {
    text: String,
    encoding: TextEncoding,
    has_bom: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TextEncoding {
    Utf8,
    Utf16Le,
    Utf16Be,
}

impl TextDocument {
    fn load(path: &Path) -> Result<Self> {
        let bytes = fs::read(path)?;
        Self::decode(&bytes)
    }

    fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xFE {
            return Ok(Self {
                text: decode_utf16le(&bytes[2..])?,
                encoding: TextEncoding::Utf16Le,
                has_bom: true,
            });
        }
        if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
            return Ok(Self {
                text: decode_utf16be(&bytes[2..])?,
                encoding: TextEncoding::Utf16Be,
                has_bom: true,
            });
        }
        if bytes.len() >= 3 && bytes[0] == 0xEF && bytes[1] == 0xBB && bytes[2] == 0xBF {
            return Ok(Self {
                text: std::str::from_utf8(&bytes[3..])
                    .map_err(|_| {
                        CoreError::InvalidData(
                            "The configuration is not valid UTF-8 text.".to_string(),
                        )
                    })?
                    .to_string(),
                encoding: TextEncoding::Utf8,
                has_bom: true,
            });
        }
        if looks_utf16(bytes, true) {
            return Ok(Self {
                text: decode_utf16le(bytes)?,
                encoding: TextEncoding::Utf16Le,
                has_bom: false,
            });
        }
        if looks_utf16(bytes, false) {
            return Ok(Self {
                text: decode_utf16be(bytes)?,
                encoding: TextEncoding::Utf16Be,
                has_bom: false,
            });
        }
        // Strict UTF-8 without BOM: mirrors UTF8Encoding(false, true), which
        // throws on invalid bytes instead of silently corrupting the text.
        match std::str::from_utf8(bytes) {
            Ok(text) => Ok(Self {
                text: text.to_string(),
                encoding: TextEncoding::Utf8,
                has_bom: false,
            }),
            Err(_) => Err(CoreError::InvalidData(
                "The configuration is not valid UTF-8 text.".to_string(),
            )),
        }
    }

    fn encoding_name(&self) -> &'static str {
        match self.encoding {
            TextEncoding::Utf8 => "utf-8",
            TextEncoding::Utf16Le => "utf-16",
            TextEncoding::Utf16Be => "utf-16BE",
        }
    }

    fn encode(&self, text: &str) -> Vec<u8> {
        let mut body = match self.encoding {
            TextEncoding::Utf8 => text.as_bytes().to_vec(),
            TextEncoding::Utf16Le => {
                let mut out = Vec::with_capacity(text.len() * 2);
                for unit in text.encode_utf16() {
                    out.extend_from_slice(&unit.to_le_bytes());
                }
                out
            }
            TextEncoding::Utf16Be => {
                let mut out = Vec::with_capacity(text.len() * 2);
                for unit in text.encode_utf16() {
                    out.extend_from_slice(&unit.to_be_bytes());
                }
                out
            }
        };
        if self.has_bom {
            let mut with_bom = match self.encoding {
                TextEncoding::Utf8 => vec![0xEF, 0xBB, 0xBF],
                TextEncoding::Utf16Le => vec![0xFF, 0xFE],
                TextEncoding::Utf16Be => vec![0xFE, 0xFF],
            };
            with_bom.append(&mut body);
            with_bom
        } else {
            body
        }
    }
}

fn decode_utf16le(bytes: &[u8]) -> Result<String> {
    let (chunks, remainder) = bytes.as_chunks::<2>();
    if !remainder.is_empty() {
        return Err(CoreError::InvalidData(
            "The configuration is not valid UTF-16 text.".to_string(),
        ));
    }
    let units: Vec<u16> = chunks.iter().map(|c| u16::from_le_bytes(*c)).collect();
    String::from_utf16(&units).map_err(|_| {
        CoreError::InvalidData("The configuration is not valid UTF-16 text.".to_string())
    })
}

fn decode_utf16be(bytes: &[u8]) -> Result<String> {
    let (chunks, remainder) = bytes.as_chunks::<2>();
    if !remainder.is_empty() {
        return Err(CoreError::InvalidData(
            "The configuration is not valid UTF-16 text.".to_string(),
        ));
    }
    let units: Vec<u16> = chunks.iter().map(|c| u16::from_be_bytes(*c)).collect();
    String::from_utf16(&units).map_err(|_| {
        CoreError::InvalidData("The configuration is not valid UTF-16 text.".to_string())
    })
}

fn looks_utf16(bytes: &[u8], little_endian: bool) -> bool {
    if bytes.len() < 4 {
        return false;
    }
    let sample = bytes.len().min(200);
    let mut zeroes = 0;
    let mut index = if little_endian { 1 } else { 0 };
    while index < sample {
        if bytes[index] == 0 {
            zeroes += 1;
        }
        index += 2;
    }
    zeroes >= sample / 8
}
