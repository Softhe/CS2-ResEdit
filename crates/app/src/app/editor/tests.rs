//! Editing transitions, exercised without constructing the UI.

use super::*;
use std::fs;

const CONFIG: &str = "\"setting.defaultres\" \"1920\"\n\"setting.defaultresheight\" \"1080\"\n\"setting.aspectratiomode\" \"1\"\n";

fn editor() -> Editor {
    Editor::new(VideoConfigService)
}

fn write_config(dir: &Path, name: &str, text: &str) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, text).unwrap();
    path
}

fn loaded(dir: &Path) -> (Editor, PathBuf) {
    let path = write_config(dir, "cs2_video.txt", CONFIG);
    let mut editor = editor();
    editor.load(path.clone()).unwrap();
    (editor, path)
}

fn set_width(editor: &mut Editor, width: &str) {
    editor.custom_width = width.to_string();
    editor.refresh();
}

#[test]
fn edit_state_tracks_no_file_invalid_unchanged_changed() {
    let dir = tempfile::tempdir().unwrap();
    let mut editor = editor();
    assert_eq!(editor.edit(), EditState::NoFile);
    assert_eq!(
        editor.apply_blocker_reason(),
        Some("Select a configuration")
    );
    assert!(!editor.has_pending_changes());

    editor
        .load(write_config(dir.path(), "cs2_video.txt", CONFIG))
        .unwrap();
    assert!(matches!(editor.edit(), EditState::Unchanged { .. }));
    assert_eq!(editor.apply_blocker_reason(), Some("No changes to apply"));
    assert!(!editor.has_pending_changes());

    set_width(&mut editor, "1280");
    assert!(matches!(editor.edit(), EditState::Changed { .. }));
    assert_eq!(editor.apply_blocker_reason(), None);
    assert!(editor.can_apply());

    set_width(&mut editor, "bogus");
    assert_eq!(editor.edit(), EditState::Invalid);
    assert_eq!(editor.edit().resolution(), None);
    assert_eq!(
        editor.apply_blocker_reason(),
        Some("Enter valid dimensions")
    );
    assert!(!editor.validation().is_empty());
    assert!(!editor.can_apply());
    assert!(editor.has_pending_changes(), "Reset stays available");

    editor.reset_pending();
    assert!(matches!(editor.edit(), EditState::Unchanged { .. }));
    assert_eq!(editor.custom_width, "1920");
}

#[test]
fn preview_keeps_last_valid_shape_while_typing() {
    let mut editor = editor();
    editor.set_loaded_state(VideoConfigState::new(1920, 1080, 1));
    editor.aspect_index = 1;
    editor.custom_width = "1280".to_string();
    editor.custom_height = "960".to_string();
    editor.refresh();
    let shape = editor.preview_resolution().expect("valid shape");
    assert_eq!((shape.width, shape.height), (1280, 960));
    // Intermediate typing state: invalid, but the preview shape persists.
    set_width(&mut editor, "12");
    assert_eq!(editor.edit(), EditState::Invalid);
    assert_eq!(editor.preview_resolution(), Some(shape));
}

#[test]
fn apply_writes_reloads_and_reports_the_backup() {
    let dir = tempfile::tempdir().unwrap();
    let (mut editor, path) = loaded(dir.path());
    set_width(&mut editor, "1280");
    let ApplyOutcome::Applied {
        backup: Some(backup),
    } = editor.apply().unwrap()
    else {
        panic!("expected a backup");
    };
    assert!(backup.is_file());
    assert_eq!(VideoConfigService.read(&path).unwrap().width, 1280);
    assert_eq!(editor.original().unwrap().width, 1280);
    assert!(!editor.has_pending_changes());
    assert!(matches!(editor.apply(), Err(WriteError::Unavailable)));

    editor.create_backup = false;
    set_width(&mut editor, "1600");
    assert_eq!(
        editor.apply().unwrap(),
        ApplyOutcome::Applied { backup: None }
    );
}

#[test]
fn apply_rejects_external_change_until_reload() {
    let dir = tempfile::tempdir().unwrap();
    let (mut editor, path) = loaded(dir.path());
    set_width(&mut editor, "1280");
    let external = format!("{CONFIG}// external edit\n");
    fs::write(&path, &external).unwrap();

    assert!(matches!(
        editor.apply(),
        Err(WriteError::ExternalChange(message)) if message.contains("Reload")
    ));
    assert!(editor
        .external_change()
        .unwrap()
        .contains("other file content changed"));
    assert_eq!(fs::read_to_string(&path).unwrap(), external);
    assert!(editor.backups().unwrap().is_empty());
    assert!(editor.has_pending_changes(), "the edit survives");

    editor.load(path.clone()).unwrap();
    assert!(editor.external_change().is_none());
    set_width(&mut editor, "1280");
    editor.apply().unwrap();
    assert_eq!(editor.original().unwrap().width, 1280);
}

#[test]
fn restore_rolls_back_and_rejects_external_change() {
    let dir = tempfile::tempdir().unwrap();
    let (mut editor, path) = loaded(dir.path());
    set_width(&mut editor, "1280");
    editor.apply().unwrap();
    let backup = PathBuf::from(&editor.backups().unwrap()[0].path);

    let changed = fs::read_to_string(&path).unwrap().replace(
        "\"setting.defaultres\" \"1280\"",
        "\"setting.defaultres\" \"1600\"",
    );
    fs::write(&path, &changed).unwrap();
    assert!(matches!(
        editor.restore(&backup),
        Err(WriteError::ExternalChange(_))
    ));
    let comparison = editor.external_change().unwrap();
    assert!(comparison.contains("Loaded: 1280 × 1080"), "{comparison}");
    assert!(comparison.contains("On disk: 1600 × 1080"), "{comparison}");
    assert_eq!(fs::read_to_string(&path).unwrap(), changed);

    editor.load(path.clone()).unwrap();
    set_width(&mut editor, "1024");
    editor.restore(&backup).unwrap();
    assert_eq!(editor.original().unwrap().width, 1920);
    assert!(!editor.has_pending_changes(), "restore discards the edit");
}

#[test]
fn failed_load_preserves_current_configuration_and_pending_changes() {
    let dir = tempfile::tempdir().unwrap();
    let (mut editor, path) = loaded(dir.path());
    let invalid = write_config(dir.path(), "invalid.txt", "not a video configuration");
    set_width(&mut editor, "1280");

    assert!(editor.load(invalid).is_err());
    assert_eq!(editor.path(), Some(path.as_path()));
    assert_eq!(editor.original().unwrap().width, 1920);
    assert_eq!(editor.custom_width, "1280");
    assert!(editor.has_pending_changes());
}

#[test]
fn aspect_selection_filters_preset_catalog() {
    let mut editor = editor();
    for (mode, expected_first, expected_ratios) in [
        (0, "1280x960", &["(4:3)", "(5:4)"][..]),
        (1, "1920x1080", &["(16:9)"][..]),
        (2, "1680x1050", &["(16:10)"][..]),
    ] {
        editor.select_aspect(mode as usize);
        let catalog_count = ResolutionCatalog::presets()
            .iter()
            .filter(|p| p.mode == mode)
            .count();
        assert_eq!(editor.preset_choices().len(), catalog_count + 1);
        for choice in &editor.preset_choices()[1..] {
            assert!(
                expected_ratios.iter().any(|r| choice.label.contains(r)),
                "unexpected label {}",
                choice.label
            );
        }
        assert!(editor.preset_choices()[editor.selected_preset()]
            .label
            .starts_with(expected_first));
        assert_eq!(
            editor.custom_width,
            expected_first.split('x').next().unwrap()
        );
    }
}

#[test]
fn reported_display_modes_are_listed_before_catalog_only_modes() {
    let mut editor = editor();
    editor.set_supported_modes([(1280, 720), (1360, 768), (1920, 1080)].into());
    editor.select_aspect(1);
    let labels: Vec<_> = editor
        .preset_choices()
        .iter()
        .map(|c| c.label.clone())
        .collect();
    assert!(labels[1].starts_with("1280x720") && labels[1].contains("available"));
    assert!(labels[2].starts_with("1360x768") && labels[2].contains("available"));
    assert!(labels.iter().any(|l| l.contains("not reported")));
}

#[test]
fn portrait_display_modes_are_not_offered_as_presets() {
    let mut editor = editor();
    editor.set_supported_modes([(1080, 1920), (1920, 1080), (1200, 1600)].into());
    for mode in 0..3 {
        editor.populate_preset_choices(mode, None, None);
        assert!(editor
            .preset_choices()
            .iter()
            .filter_map(|choice| choice.resolution)
            .all(|r| r.width >= r.height));
    }
}

#[test]
fn availability_reflects_the_selected_display() {
    let dir = tempfile::tempdir().unwrap();
    let (mut editor, _) = loaded(dir.path());
    assert_eq!(editor.availability(), Availability::NoModes);
    editor.set_supported_modes([(1920, 1080)].into());
    assert_eq!(editor.availability(), Availability::Reported);
    set_width(&mut editor, "1280");
    assert_eq!(editor.availability(), Availability::NotReported);
    set_width(&mut editor, "x");
    assert_eq!(editor.availability(), Availability::Unknown);
}

#[test]
fn external_change_comparison_distinguishes_display_and_other_edits() {
    let loaded = VideoConfigState::new(1920, 1080, 1);
    let changed = VideoConfigState::new(1280, 960, 0);
    let display_change = describe_external_change(loaded, Ok(changed));
    assert!(display_change.contains("Loaded: 1920 × 1080"));
    assert!(display_change.contains("On disk: 1280 × 960"));
    assert!(display_change.contains("4:3"));

    let other_change = describe_external_change(loaded, Ok(loaded));
    assert!(other_change.contains("display settings are the same"));
    assert!(other_change.contains("other file content changed"));

    let unreadable = describe_external_change(
        loaded,
        Err(CoreError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "missing",
        ))),
    );
    assert!(unreadable.contains("could not be read"));
    let invalid =
        describe_external_change(loaded, Err(CoreError::InvalidData("invalid".to_string())));
    assert!(invalid.contains("no longer has valid display settings"));
}
