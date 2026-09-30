//! Behavioral parity tests ported from `tests/CS2ResEdit.Tests` (xUnit).
//!
//! Each test mirrors its C# counterpart: VDF parsing, resolution catalog,
//! video configuration updates/backups, Steam discovery, preferences, display
//! models, and privacy-safe diagnostics.

use cs2_resedit_core::{
    CoreError, DiagnosticService, DisplayInfo, DisplayResolution, PreferencesService,
    ResolutionCatalog, SteamService, ValveKeyValues, VideoConfigService,
};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::Duration;

const CONFIG: &str = "\"setting.defaultres\"\t\t\"1920\"\r\n\"setting.defaultresheight\"\t\t\"1080\"\r\n\"setting.aspectratiomode\"\t\t\"1\"\r\n\"unrelated\"\t\t\"keep\"\r\n";

fn service() -> VideoConfigService {
    VideoConfigService
}

#[test]
fn themes_round_trip_and_missing_or_unknown_themes_default_to_glacier() {
    use cs2_resedit_core::UiTheme;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    let preferences = PreferencesService::new(service());
    assert_eq!(preferences.read(Some(&path)).theme, UiTheme::Glacier);
    for theme in UiTheme::ALL {
        let saved =
            preferences.save_selection_with_theme(Some("123"), None, &[], theme, Some(&path));
        assert!(saved.warning.is_none());
        assert_eq!(preferences.read(Some(&path)).theme, theme);
        preferences.save_selection(Some("456"), None, &[], Some(&path));
        let loaded = preferences.read(Some(&path));
        assert_eq!(loaded.theme, theme);
        assert_eq!(loaded.last_account_id.as_deref(), Some("456"));
    }
    for json in [
        r#"{"SchemaVersion":1}"#,
        r#"{"SchemaVersion":1,"Theme":"FutureTheme","LastAccountId":"123"}"#,
    ] {
        fs::write(&path, json).unwrap();
        let loaded = preferences.read(Some(&path));
        assert_eq!(loaded.theme, UiTheme::Glacier);
        assert!(loaded.warning.is_none());
    }
}

fn write_config(dir: &Path) -> PathBuf {
    let path = dir.join("cs2_video.txt");
    fs::write(&path, CONFIG.as_bytes()).expect("write config");
    path
}

// ---------- VideoConfig ----------

#[test]
fn reads_all_required_fields() {
    let state = service().parse(CONFIG).expect("parse");
    assert_eq!(state.width, 1920);
    assert_eq!(state.height, 1080);
    assert_eq!(state.aspect_mode, 1);
}

#[test]
fn rejects_missing_and_duplicate_fields() {
    let missing = CONFIG.replace("\"setting.defaultresheight\"\t\t\"1080\"\r\n", "");
    assert!(service().parse(&missing).is_err());
    let duplicate = format!("{CONFIG}\"setting.defaultres\"\t\"800\"\r\n");
    assert!(service().parse(&duplicate).is_err());
}

#[test]
fn rejects_malformed_setting_suffix_without_writing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("cs2_video.txt");
    let malformed = CONFIG.replace("\"1920\"", "\"1920\" trailing-data");
    fs::write(&path, &malformed).expect("write config");
    let resolution = ResolutionCatalog::parse("1280x720", None).expect("resolution");
    assert!(service().update(&path, &resolution, true).is_err());
    assert_eq!(fs::read_to_string(&path).expect("read config"), malformed);
    assert!(service().get_backups(&path).expect("backups").is_empty());
}

#[test]
fn guarded_update_rejects_external_change_and_preserves_unrelated_content() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_config(dir.path());
    let snapshot = service().read_snapshot(&path).expect("snapshot");
    let externally_changed = format!("{CONFIG}\"external.setting\" \"keep me\"\r\n");
    fs::write(&path, &externally_changed).expect("external change");
    let resolution = ResolutionCatalog::parse("1280x720", None).expect("resolution");
    let err = service()
        .update_if_unchanged(&path, &resolution, true, &snapshot)
        .expect_err("must reject external change");
    assert!(matches!(err, CoreError::ExternalChange(_)));
    assert_eq!(err.category(), "InvalidDataException");
    assert!(err.to_string().contains("Reload"));
    assert_eq!(fs::read_to_string(&path).expect("read"), externally_changed);
    assert!(service().get_backups(&path).expect("backups").is_empty());

    let reloaded = service().read_snapshot(&path).expect("reload");
    service()
        .update_if_unchanged(&path, &resolution, true, &reloaded)
        .expect("update after reload");
    let output = fs::read_to_string(&path).expect("read updated");
    assert!(output.contains("\"external.setting\" \"keep me\""));
    assert_eq!(service().read(&path).expect("state").width, 1280);
}

#[test]
fn guarded_restore_rejects_external_change() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_config(dir.path());
    let resolution = ResolutionCatalog::parse("1280x720", None).expect("resolution");
    let updated = service().update(&path, &resolution, true).expect("update");
    let backup = Path::new(updated.backup_path.as_deref().expect("backup"));
    let snapshot = service().read_snapshot(&path).expect("snapshot");
    let externally_changed = format!(
        "{}\"external.setting\" \"keep me\"\r\n",
        fs::read_to_string(&path).expect("read")
    );
    fs::write(&path, &externally_changed).expect("external change");
    let err = service()
        .restore_if_unchanged(&path, backup, &snapshot)
        .expect_err("must reject external change");
    assert!(matches!(err, CoreError::ExternalChange(_)));
    assert!(err.to_string().contains("Reload"));
    assert_eq!(fs::read_to_string(&path).expect("read"), externally_changed);
}

#[test]
fn rejects_malformed_utf16_without_writing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let resolution = ResolutionCatalog::parse("1280x720", None).expect("resolution");
    for bytes in [vec![0xFF, 0xFE, 0x00], vec![0xFF, 0xFE, 0x00, 0xD8]] {
        let path = dir.path().join("cs2_video.txt");
        fs::write(&path, &bytes).expect("write config");
        assert!(service().update(&path, &resolution, true).is_err());
        assert_eq!(fs::read(&path).expect("read config"), bytes);
    }
}

#[test]
fn rejects_invalid_utf8_after_bom_without_writing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("cs2_video.txt");
    let mut bytes = vec![0xEF, 0xBB, 0xBF];
    bytes.extend_from_slice(CONFIG.as_bytes());
    bytes.extend_from_slice(b"\xff corrupt unrelated tail");
    fs::write(&path, &bytes).expect("write config");
    let resolution = ResolutionCatalog::parse("1280x720", None).expect("resolution");

    assert!(service().update(&path, &resolution, true).is_err());
    assert_eq!(fs::read(&path).expect("read config"), bytes);
    assert!(service().get_backups(&path).expect("backups").is_empty());
}

#[test]
fn preserves_encoding_bom_and_line_endings() {
    let cases = [
        ("utf8", false),
        ("utf8", true),
        ("utf16le", false),
        ("utf16le", true),
        ("utf16be", false),
        ("utf16be", true),
    ];
    for (encoding_name, bom) in cases {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(format!("{encoding_name}-{bom}.txt"));
        // Mirrors the C# test: the BOM flag selects LF vs CRLF source text.
        let text = if bom {
            CONFIG.replace("\r\n", "\n")
        } else {
            CONFIG.to_string()
        };
        let bytes: Vec<u8> = match (encoding_name, bom) {
            ("utf16le", true) => {
                let mut out = vec![0xFF, 0xFE];
                for unit in text.encode_utf16() {
                    out.extend_from_slice(&unit.to_le_bytes());
                }
                out
            }
            ("utf16le", false) => {
                let mut out = Vec::new();
                for unit in text.encode_utf16() {
                    out.extend_from_slice(&unit.to_le_bytes());
                }
                out
            }
            ("utf16be", true) => {
                let mut out = vec![0xFE, 0xFF];
                for unit in text.encode_utf16() {
                    out.extend_from_slice(&unit.to_be_bytes());
                }
                out
            }
            ("utf16be", false) => {
                let mut out = Vec::new();
                for unit in text.encode_utf16() {
                    out.extend_from_slice(&unit.to_be_bytes());
                }
                out
            }
            (_, true) => {
                let mut out = vec![0xEF, 0xBB, 0xBF];
                out.extend_from_slice(text.as_bytes());
                out
            }
            _ => text.as_bytes().to_vec(),
        };
        fs::write(&path, &bytes).expect("write encoded");

        let resolution = ResolutionCatalog::parse("1280x720", None).expect("parse resolution");
        service().update(&path, &resolution, false).expect("update");

        let output = fs::read(&path).expect("read back");
        let preamble: &[u8] = match (encoding_name, bom) {
            ("utf16le", true) => &[0xFF, 0xFE],
            ("utf16be", true) => &[0xFE, 0xFF],
            ("utf8", true) => &[0xEF, 0xBB, 0xBF],
            _ => &[],
        };
        assert_eq!(
            bom && !preamble.is_empty(),
            output.starts_with(preamble) && !preamble.is_empty(),
            "BOM round-trip for {encoding_name} bom={bom}"
        );
        let body = &output[if bom { preamble.len() } else { 0 }..];
        let decoded = match encoding_name {
            "utf16le" => {
                let (chunks, _) = body.as_chunks::<2>();
                String::from_utf16_lossy(
                    &chunks
                        .iter()
                        .map(|c| u16::from_le_bytes(*c))
                        .collect::<Vec<_>>(),
                )
            }
            "utf16be" => {
                let (chunks, _) = body.as_chunks::<2>();
                String::from_utf16_lossy(
                    &chunks
                        .iter()
                        .map(|c| u16::from_be_bytes(*c))
                        .collect::<Vec<_>>(),
                )
            }
            _ => String::from_utf8_lossy(body).to_string(),
        };
        assert!(
            decoded.contains(if bom { "\n" } else { "\r\n" }),
            "line endings preserved for {encoding_name} bom={bom}"
        );
        assert!(decoded.contains("\"unrelated\"\t\t\"keep\""));
        let state = service().read(&path).expect("read state");
        assert_eq!(
            (state.width, state.height, state.aspect_mode),
            (1280, 720, 1)
        );
    }
}

#[test]
fn creates_restores_and_retains_recognized_backups() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_config(dir.path());
    for i in 0..7 {
        let resolution =
            ResolutionCatalog::parse(&format!("{}x720", 1280 + i), None).expect("parse");
        service().update(&path, &resolution, true).expect("update");
        thread::sleep(Duration::from_millis(10));
    }
    fs::write(dir.path().join("cs2_video.txt.manual.bak"), "do not delete").expect("manual backup");
    let backups = service().get_backups(&path).expect("backups");
    assert_eq!(backups.len(), 5);
    assert!(dir.path().join("cs2_video.txt.manual.bak").exists());
    let restore = backups.last().expect("oldest retained").clone();
    let rollback = service()
        .restore(&path, Path::new(&restore.path))
        .expect("restore");
    assert!(rollback.is_file());
    assert_eq!(service().read(&path).expect("read"), restore.state);
}

#[cfg(windows)]
#[test]
fn update_reports_prune_failure_and_restores_configuration() {
    use std::os::windows::fs::OpenOptionsExt;
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_config(dir.path());
    for i in 0..5 {
        let resolution =
            ResolutionCatalog::parse(&format!("{}x720", 1280 + i), None).expect("resolution");
        service().update(&path, &resolution, true).expect("update");
    }
    let backups = service().get_backups(&path).expect("backups");
    let oldest = Path::new(&backups.last().expect("oldest").path);
    let _lock = fs::OpenOptions::new()
        .read(true)
        .share_mode(0x00000001)
        .open(oldest)
        .expect("hold backup without delete sharing");
    let before = fs::read(&path).expect("read original");
    let resolution = ResolutionCatalog::parse("1400x720", None).expect("resolution");
    assert!(service().update(&path, &resolution, true).is_err());
    assert_eq!(fs::read(&path).expect("read restored"), before);
}

#[cfg(windows)]
#[test]
fn restore_reports_prune_failure_and_preserves_configuration() {
    use std::os::windows::fs::OpenOptionsExt;
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_config(dir.path());
    for i in 0..5 {
        let resolution =
            ResolutionCatalog::parse(&format!("{}x720", 1280 + i), None).expect("resolution");
        service().update(&path, &resolution, true).expect("update");
    }
    let backups = service().get_backups(&path).expect("backups");
    let oldest = Path::new(&backups.last().expect("oldest").path);
    let _lock = fs::OpenOptions::new()
        .read(true)
        .share_mode(0x00000001)
        .open(oldest)
        .expect("hold backup without delete sharing");
    let before = fs::read(&path).expect("read original");
    assert!(service().restore(&path, oldest).is_err());
    assert_eq!(fs::read(&path).expect("read restored"), before);
}

#[test]
fn backup_names_are_validated_before_listing() {
    // Locks in the regex-free backup-name validator: timestamp shape,
    // optional -N collision suffix, case-insensitive extension.
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_config(dir.path());
    let valid = [
        "cs2_video.txt.20240101-010101.bak",
        "cs2_video.txt.20240101-010101-3.bak",
        "cs2_video.txt.20240101-010102.BAK",
    ];
    let rejected = [
        "cs2_video.txt.manual.bak",
        "cs2_video.txt.old.20240101-010101.bak",
        "cs2_video.txt.20240101-0101.bak",
        "cs2_video.txt.20240101-010101x.bak",
        "cs2_video.txt.x0240101-010101.bak",
    ];
    for name in valid.iter().chain(rejected.iter()) {
        fs::write(dir.path().join(name), valid_config_text()).expect("write backup fixture");
    }
    let backups = service().get_backups(&path).expect("backups");
    let mut listed: Vec<String> = backups
        .iter()
        .map(|b| {
            Path::new(&b.path)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default()
        })
        .collect();
    listed.sort_unstable();
    let mut expected = valid.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    expected.sort_unstable();
    assert_eq!(listed, expected);
}

#[test]
fn non_ascii_backup_name_is_ignored_without_panicking() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_config(dir.path());
    fs::write(
        dir.path().join("cs2_video.txt.12345678-1234éxx.bak"),
        valid_config_text(),
    )
    .expect("write malformed backup");

    assert!(service().get_backups(&path).expect("backups").is_empty());
}

#[test]
fn retention_counts_corrupt_owned_backups() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_config(dir.path());
    for second in 0..6 {
        fs::write(
            dir.path()
                .join(format!("cs2_video.txt.20240101-0101{second:02}.bak")),
            if second == 0 {
                "corrupt"
            } else {
                valid_config_text()
            },
        )
        .expect("write backup");
    }
    let resolution = ResolutionCatalog::parse("1280x720", None).expect("resolution");
    service().update(&path, &resolution, true).expect("update");

    let owned_count = fs::read_dir(dir.path())
        .expect("read dir")
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            name.starts_with("cs2_video.txt.20") && name.ends_with(".bak")
        })
        .count();
    assert_eq!(owned_count, 5);
}

#[test]
fn concurrent_guarded_updates_allow_only_one_snapshot_writer() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_config(dir.path());
    let snapshot = service().read_snapshot(&path).expect("snapshot");
    let barrier = Arc::new(Barrier::new(3));
    let mut handles = Vec::new();
    for value in ["1280x720", "1600x900"] {
        let path = path.clone();
        let snapshot = snapshot.clone();
        let barrier = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            let resolution = ResolutionCatalog::parse(value, None).expect("resolution");
            barrier.wait();
            service().update_if_unchanged(&path, &resolution, true, &snapshot)
        }));
    }
    barrier.wait();
    let results: Vec<_> = handles
        .into_iter()
        .map(|handle| handle.join().expect("join"))
        .collect();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, Err(CoreError::ExternalChange(_))))
            .count(),
        1
    );
}

#[test]
fn atomic_write_replaces_target_content() {
    // Regression guard for the ReplaceFileW argument order: a swapped call
    // deletes the live target and leaves the update "successful", which the
    // post-write verification then reports as os error 2 (file not found).
    let dir = tempfile::tempdir().expect("tempdir");
    let target = dir.path().join("target.txt");
    fs::write(&target, b"old-bytes").expect("seed target");

    cs2_resedit_core::fsutil::atomic_write(&target, b"new-bytes").expect("atomic write");

    // The target must survive and hold the new content.
    assert_eq!(
        fs::read(&target).expect("read back"),
        b"new-bytes" as &[u8],
        "target content was not replaced"
    );
    // No leftover temporary files beside it.
    let leftovers: Vec<String> = fs::read_dir(dir.path())
        .expect("read dir")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .filter(|name| name.ends_with(".tmp"))
        .collect();
    assert!(leftovers.is_empty(), "leftover temp files: {leftovers:?}");
}

#[test]
#[cfg(windows)]
fn native_paths_use_backslashes() {
    let native = cs2_resedit_core::fsutil::absolute_lossy(Path::new("c:/temp/x.txt"));
    assert!(
        !native.to_string_lossy().contains('/'),
        "expected native separators, got {native:?}"
    );
}

#[test]
fn does_not_write_when_state_is_unchanged() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_config(dir.path());
    let before = fs::metadata(&path)
        .expect("metadata")
        .modified()
        .expect("mtime");
    let resolution = ResolutionCatalog::parse("1920x1080", None).expect("parse");
    let result = service().update(&path, &resolution, true).expect("update");
    assert!(!result.changed);
    assert!(service().get_backups(&path).expect("backups").is_empty());
    assert_eq!(
        fs::metadata(&path)
            .expect("metadata")
            .modified()
            .expect("mtime"),
        before
    );
}

// ---------- Resolution ----------

#[test]
fn parses_valid_dimensions() {
    for (input, width, height, mode) in [
        ("1920x1080", 1920, 1080, 1),
        (" 1280 X 1024 ", 1280, 1024, 0),
        ("1440×900", 1440, 900, 2),
    ] {
        let value = ResolutionCatalog::parse(input, None).expect("parse");
        assert_eq!(
            (value.width, value.height, value.mode),
            (width, height, mode)
        );
    }
}

#[test]
fn rejects_invalid_dimensions() {
    for input in ["", "1920", "1x1", "40000x1080"] {
        assert!(ResolutionCatalog::parse(input, None).is_err(), "{input}");
    }
}

#[test]
fn preset_catalog_is_expanded_and_has_unique_dimensions() {
    let presets = ResolutionCatalog::presets();
    assert!(presets.len() >= 40, "got {}", presets.len());
    let unique: HashSet<(i32, i32)> = presets.iter().map(|p| (p.width, p.height)).collect();
    assert_eq!(unique.len(), presets.len());
}

#[test]
fn contains_common_resolution_presets() {
    for (width, height, mode) in [
        (800, 600, 0),
        (1024, 768, 0),
        (1920, 1080, 1),
        (2560, 1440, 1),
        (1280, 800, 2),
        (1920, 1200, 2),
    ] {
        assert!(
            ResolutionCatalog::presets()
                .iter()
                .any(|p| p.width == width && p.height == height && p.mode == mode),
            "{width}x{height} mode {mode}"
        );
    }
}

#[test]
fn provides_recommended_preset_for_each_aspect_mode() {
    for (mode, width, height) in [(0, 1280, 960), (1, 1920, 1080), (2, 1680, 1050)] {
        let preset = ResolutionCatalog::recommended_preset(mode).expect("recommended");
        assert_eq!(
            (preset.width, preset.height, preset.mode),
            (width, height, mode)
        );
    }
}

// ---------- Steam & preferences ----------

fn valid_config_text() -> &'static str {
    "\"setting.defaultres\" \"1920\"\n\"setting.defaultresheight\" \"1080\"\n\"setting.aspectratiomode\" \"1\"\n"
}

#[test]
fn converts_account_id() {
    assert_eq!(
        SteamService::to_steam_id64(123_456_867),
        76_561_198_083_722_595
    );
}

#[test]
fn discovers_and_orders_steam_accounts() {
    let dir = tempfile::tempdir().expect("tempdir");
    let config = dir
        .path()
        .join("userdata/123456867/730/local/cfg/cs2_video.txt");
    fs::create_dir_all(config.parent().expect("parent")).expect("mkdir");
    fs::write(&config, valid_config_text()).expect("write");
    fs::create_dir_all(dir.path().join("config")).expect("mkdir");
    fs::write(
        dir.path().join("config/loginusers.vdf"),
        "\"users\"\n{\n\"76561198083722595\"\n{\n\"AccountName\" \"tester\"\n\"PersonaName\" \"Test Person\"\n\"MostRecent\" \"1\"\n}\n}",
    )
    .expect("write vdf");
    let accounts = SteamService.get_accounts(&[dir.path().to_string_lossy().to_string()]);
    assert_eq!(accounts.len(), 1);
    assert_eq!(accounts[0].persona_name, "Test Person");
    assert!(accounts[0].most_recent);
    assert!(accounts[0].has_config);
}

#[test]
fn parses_nested_escaped_and_duplicate_login_users() {
    let dir = tempfile::tempdir().expect("tempdir");
    let login_users = dir.path().join("loginusers.vdf");
    fs::write(
        &login_users,
        "// Steam metadata\n\"users\"\n{\n\"76561198083722595\"\n{\n\"AccountName\" \"tester\"\n\"PersonaName\" \"First\"\n\"MostRecent\" \"0\"\n}\n\"76561198083722595\"\n{\n\"AccountName\" \"tester\"\n\"PersonaName\" \"Test \\\"Player\\\"\"\n\"MostRecent\" \"1\"\n\"nested\" { \"ignored\" \"yes\" }\n}\n}\n",
    )
    .expect("write");
    let users = SteamService.parse_login_users(&login_users).expect("parse");
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].persona_name, "Test \"Player\"");
    assert!(users[0].most_recent);
}

#[test]
fn malformed_login_users_does_not_break_discovery() {
    let dir = tempfile::tempdir().expect("tempdir");
    let login_users = dir.path().join("malformed.vdf");
    fs::write(&login_users, "\"users\" { \"123\" {").expect("write");
    assert!(SteamService.parse_login_users(&login_users).is_err());
    assert!(SteamService
        .get_accounts(&[dir.path().to_string_lossy().to_string()])
        .is_empty());
}

#[test]
fn malformed_login_users_reports_degraded_discovery() {
    let dir = tempfile::tempdir().expect("tempdir");
    fs::create_dir_all(dir.path().join("config")).expect("config directory");
    fs::create_dir_all(dir.path().join("userdata/123456867")).expect("userdata directory");
    fs::write(dir.path().join("config/loginusers.vdf"), "\"users\" {").expect("malformed file");
    let (accounts, warnings) =
        SteamService.get_accounts_with_warnings(&[dir.path().to_string_lossy().to_string()]);
    assert_eq!(accounts.len(), 1);
    assert_eq!(warnings.len(), 1);
    assert!(accounts[0].persona_name.starts_with("Steam account"));
}

#[test]
fn login_users_with_utf8_bom_are_parsed() {
    let dir = tempfile::tempdir().expect("tempdir");
    let login_users = dir.path().join("loginusers.vdf");
    let mut bytes = vec![0xEF, 0xBB, 0xBF];
    bytes.extend_from_slice(
        b"\"users\"\n{\n\"76561198083722595\"\n{\n\"PersonaName\" \"Bom User\"\n\"MostRecent\" \"1\"\n}\n}",
    );
    fs::write(&login_users, bytes).expect("write");
    let users = SteamService.parse_login_users(&login_users).expect("parse");
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].persona_name, "Bom User");
    assert!(users[0].most_recent);
}

#[test]
fn userdata_files_are_not_treated_as_accounts() {
    // C# enumerates directories only; a stray file named like an account ID
    // must not appear in discovery results.
    let dir = tempfile::tempdir().expect("tempdir");
    fs::create_dir_all(dir.path().join("userdata")).expect("mkdir");
    fs::write(dir.path().join("userdata/123456867"), "not a directory").expect("write");
    let accounts = SteamService.get_accounts(&[dir.path().to_string_lossy().to_string()]);
    assert!(accounts.is_empty());
}

#[test]
fn valve_keyvalues_rejects_trailing_content() {
    assert!(ValveKeyValues::parse("\"a\" \"1\" \"b\" \"2\" extra").is_err());
}

#[test]
fn valve_keyvalues_rejects_excessive_nesting() {
    let mut text = String::new();
    for _ in 0..130 {
        text.push_str("\"key\" {");
    }
    text.push_str("\"value\" \"1\"");
    for _ in 0..130 {
        text.push('}');
    }
    let error = ValveKeyValues::parse(&text).expect_err("nesting must be bounded");
    assert!(error.to_string().contains("too deep"));
}

#[test]
fn aspect_helpers_reject_invalid_dimensions() {
    assert!(ResolutionCatalog::automatic_aspect_mode(1920, 0).is_err());
    assert!(ResolutionCatalog::mode_name(1, 1920, 0).is_err());
}

#[test]
fn reads_schema_one_and_cleans_recent_files() {
    let dir = tempfile::tempdir().expect("tempdir");
    let config = dir.path().join("cs2_video.txt");
    fs::write(&config, valid_config_text()).expect("write");
    let settings = dir.path().join("settings.json");
    let escaped = config.to_string_lossy().replace('\\', "\\\\");
    fs::write(
        &settings,
        format!(
            "{{\"SchemaVersion\":1,\"LastAccountId\":\"42\",\"RecentConfigPaths\":[\"{escaped}\",\"missing\"]}}"
        ),
    )
    .expect("write");
    let configs = service();
    let prefs_service = PreferencesService::new(configs);
    let value = prefs_service.read(Some(&settings));
    assert_eq!(value.last_account_id.as_deref(), Some("42"));
    assert_eq!(value.recent_config_paths.len(), 1);

    prefs_service.save(
        Some("43"),
        &[config.to_string_lossy().to_string()],
        Some(&settings),
    );
    assert_eq!(
        prefs_service
            .read(Some(&settings))
            .last_account_id
            .as_deref(),
        Some("43")
    );
}

#[test]
fn selected_config_path_round_trips_and_old_schema_remains_valid() {
    let dir = tempfile::tempdir().expect("tempdir");
    let config = dir.path().join("custom_cs2_video.txt");
    fs::write(&config, valid_config_text()).expect("write config");
    let settings = dir.path().join("settings.json");
    let service = PreferencesService::new(service());

    service.save_selection(None, Some(&config), &[], Some(&settings));
    let saved = service.read(Some(&settings));
    assert_eq!(
        saved.last_config_path.as_deref(),
        Some(config.to_string_lossy().as_ref())
    );

    fs::write(&settings, r#"{"SchemaVersion":1,"LastAccountId":"42"}"#)
        .expect("write old preferences");
    let old = service.read(Some(&settings));
    assert!(old.warning.is_none());
    assert!(old.last_config_path.is_none());
}

#[test]
fn reports_unsupported_preference_schema() {
    let dir = tempfile::tempdir().expect("tempdir");
    let settings = dir.path().join("settings.json");
    fs::write(&settings, "{\"SchemaVersion\":2}").expect("write");
    let configs = service();
    let value = PreferencesService::new(configs).read(Some(&settings));
    assert!(value.warning.is_some());
    assert!(value.recent_config_paths.is_empty());
}

#[test]
fn reports_preference_save_failure() {
    let dir = tempfile::tempdir().expect("tempdir");
    let occupied_parent = dir.path().join("occupied");
    fs::write(&occupied_parent, b"file").expect("write parent blocker");
    let settings = occupied_parent.join("settings.json");
    let result = PreferencesService::new(service()).save(Some("43"), &[], Some(&settings));
    assert!(result
        .warning
        .as_deref()
        .is_some_and(|text| text.contains("could not be saved")));
    assert!(!settings.exists());
}

#[test]
fn accepts_missing_schema_version_as_one() {
    // System.Text.Json keeps the C# property initializer default of 1 when
    // the property is absent; the port must accept it too.
    let dir = tempfile::tempdir().expect("tempdir");
    let config = dir.path().join("cs2_video.txt");
    fs::write(&config, valid_config_text()).expect("write");
    let settings = dir.path().join("settings.json");
    let escaped = config.to_string_lossy().replace('\\', "\\\\");
    fs::write(
        &settings,
        format!("{{\"LastAccountId\":\"77\",\"RecentConfigPaths\":[\"{escaped}\"]}}"),
    )
    .expect("write");
    let configs = service();
    let value = PreferencesService::new(configs).read(Some(&settings));
    assert!(value.warning.is_none());
    assert_eq!(value.last_account_id.as_deref(), Some("77"));
    assert_eq!(value.recent_config_paths.len(), 1);
}

#[test]
fn reads_settings_json_with_utf8_bom() {
    let dir = tempfile::tempdir().expect("tempdir");
    let settings = dir.path().join("settings.json");
    let mut bytes = vec![0xEF, 0xBB, 0xBF];
    bytes.extend_from_slice(b"{\"SchemaVersion\":1,\"LastAccountId\":\"7\"}");
    fs::write(&settings, bytes).expect("write");
    let configs = service();
    let value = PreferencesService::new(configs).read(Some(&settings));
    assert!(value.warning.is_none());
    assert_eq!(value.last_account_id.as_deref(), Some("7"));
}

#[test]
fn rejects_malformed_typed_preference_fields() {
    let configs = service();
    let cases = [
        "{\"SchemaVersion\":null}".to_string(),
        "{\"SchemaVersion\":\"1\"}".to_string(),
        "{\"LastAccountId\":42}".to_string(),
        "{\"RecentConfigPaths\":\"x\"}".to_string(),
        "[1,2,3]".to_string(),
    ];
    for case in cases {
        let dir = tempfile::tempdir().expect("tempdir");
        let settings = dir.path().join("settings.json");
        fs::write(&settings, case.as_str()).expect("write");
        let value = PreferencesService::new(configs).read(Some(&settings));
        assert!(value.warning.is_some(), "expected warning for {case}");
        assert!(value.recent_config_paths.is_empty());
    }
}

#[test]
fn rejects_configuration_with_invalid_utf8_bytes() {
    // Mirrors UTF8Encoding(false, true): invalid bytes must surface as an
    // error instead of being silently rewritten with replacement characters.
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("cs2_video.txt");
    let mut bytes = CONFIG.as_bytes().to_vec();
    bytes.extend_from_slice(b"\xff\xfe corrupt tail");
    fs::write(&path, bytes).expect("write");
    let configs = service();
    assert!(configs.read(&path).is_err());
    assert!(configs.inspect(&path).is_err());
}

#[test]
fn uses_versioned_default_preference_directory() {
    let expected = Path::new("Softhe")
        .join("CS2-ResEdit")
        .join("v1")
        .join("settings.json")
        .to_string_lossy()
        .to_string();
    let actual = PreferencesService::default_path()
        .to_string_lossy()
        .to_string();
    assert!(
        actual.to_lowercase().ends_with(&expected.to_lowercase()),
        "default path {actual} should end with {expected}"
    );
}

#[test]
fn fresh_v1_preferences_ignore_and_preserve_previous_files() {
    let dir = tempfile::tempdir().expect("tempdir");
    let previous_product = dir.path().join("CS2-ResEdit/settings.json");
    let previous_name = dir.path().join("CS2-VideoConfig-Editor/settings.json");
    let v1 = dir.path().join("CS2-ResEdit/v1/settings.json");
    fs::create_dir_all(previous_product.parent().expect("parent")).expect("mkdir");
    fs::create_dir_all(previous_name.parent().expect("parent")).expect("mkdir");
    const PREVIOUS: &str = "{\"SchemaVersion\":1,\"LastAccountId\":\"previous\"}";
    fs::write(&previous_product, PREVIOUS).expect("write");
    fs::write(&previous_name, PREVIOUS).expect("write");

    let configs = service();
    let value = PreferencesService::new(configs).read(Some(&v1));
    assert_eq!(value.last_account_id, None);
    assert!(!v1.exists());
    assert_eq!(
        fs::read_to_string(&previous_product).expect("read"),
        PREVIOUS
    );
    assert_eq!(fs::read_to_string(&previous_name).expect("read"), PREVIOUS);
}

// ---------- Display & diagnostics ----------

#[test]
fn diagnostic_report_is_versioned_and_privacy_safe() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir
        .path()
        .join("personal-account-76561198000000000/cs2_video.txt");
    fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    fs::write(
        &path,
        "\"setting.defaultres\" \"1920\"\r\n\"setting.defaultresheight\" \"1080\"\r\n\"setting.aspectratiomode\" \"1\"\r\n",
    )
    .expect("write");
    let displays = [DisplayInfo {
        device_name: "private-device-name".to_string(),
        friendly_name: "Personal Monitor".to_string(),
        is_primary: true,
        current_width: 2560,
        current_height: 1440,
        modes: vec![
            DisplayResolution::new(1920, 1080),
            DisplayResolution::new(2560, 1440),
        ],
    }];
    let configs = service();
    let report = DiagnosticService::create(&configs, "1.0.0", &displays, 1, 2, Some(&path));
    let json = DiagnosticService::to_json(&report);
    let summary = DiagnosticService::to_summary(&report);

    assert_eq!(report.schema_version, 1);
    assert_eq!(report.configuration_status, "Valid");
    assert_eq!(report.configuration_line_ending.as_deref(), Some("CRLF"));
    assert!(summary.contains("1.0.0"));
    let lowered = json.to_lowercase();
    assert!(!lowered.contains(&path.to_string_lossy().to_lowercase()));
    assert!(!lowered.contains("personal-account"));
    assert!(!json.contains("76561198000000000"));
    assert!(!json.contains("Personal Monitor"));
    assert!(!json.contains("private-device-name"));
    assert!(!json.contains("setting.defaultres"));
}

#[test]
fn diagnostic_report_sanitizes_configuration_failure() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("secret-path.txt");
    fs::write(&path, "invalid").expect("write");
    let configs = service();
    let report = DiagnosticService::create(&configs, "1.0.0", &[], 0, 0, Some(&path));
    let json = DiagnosticService::to_json(&report);
    assert_eq!(report.configuration_status, "Invalid");
    assert_eq!(
        report.error_category.as_deref(),
        Some("InvalidDataException")
    );
    assert!(!json
        .to_lowercase()
        .contains(&path.to_string_lossy().to_lowercase()));
}

#[test]
fn display_models_expose_neutral_labels_and_unique_modes() {
    let display = DisplayInfo {
        device_name: "DISPLAY1".to_string(),
        friendly_name: "Monitor".to_string(),
        is_primary: true,
        current_width: 1920,
        current_height: 1080,
        modes: vec![
            DisplayResolution::new(1280, 720),
            DisplayResolution::new(1920, 1080),
        ],
    };
    assert!(display.label().contains("Primary"));
    assert_eq!(display.modes[1].display(), "1920x1080");
}
