//! Editing state for one configuration file: the loaded snapshot, the
//! pending resolution, and the guarded apply and restore operations.
//!
//! `Editor` has no egui or preference dependencies. Its operations return
//! outcomes, and `ResEditApp` decides what to show and what to remember.

use cs2_resedit_core::{
    BackupInfo, CoreError, Resolution, ResolutionCatalog, UpdateResult, VideoConfigService,
    VideoConfigSnapshot, VideoConfigState,
};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Explicit edit lifecycle for the loaded configuration. `None`-style
/// sentinels used to mean both "no file" and "invalid input"; each state is
/// now named.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EditState {
    /// No configuration loaded; nothing to compare against.
    NoFile,
    /// A configuration is loaded but the custom dimensions do not parse.
    /// Reset stays enabled so the user can return to the loaded values;
    /// Apply is blocked.
    Invalid,
    /// Pending values match the loaded file.
    Unchanged { resolution: Resolution },
    /// Pending values differ from the loaded file; Apply is available.
    Changed { resolution: Resolution },
}

impl EditState {
    pub(crate) fn resolution(self) -> Option<Resolution> {
        match self {
            Self::Unchanged { resolution } | Self::Changed { resolution } => Some(resolution),
            Self::NoFile | Self::Invalid => None,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct PresetChoice {
    pub(crate) resolution: Option<Resolution>,
    pub(crate) label: String,
}

/// Whether the pending resolution is one the selected display reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Availability {
    /// No file, or the custom dimensions are invalid.
    Unknown,
    /// Windows reported no modes for the selected display.
    NoModes,
    Reported,
    NotReported,
}

impl Availability {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Unknown => "",
            Self::NoModes => "Display modes unavailable",
            // U+2714 is bundled; U+2713 is tofu.
            Self::Reported => "✔ Reported for selected display",
            Self::NotReported => "Not reported; custom use is still allowed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ApplyOutcome {
    Applied {
        backup: Option<PathBuf>,
    },
    /// The file already had the pending values; nothing was written.
    AlreadyCurrent,
}

#[derive(Debug)]
pub(crate) enum WriteError {
    /// Nothing to write: no file, or no valid pending change.
    Unavailable,
    /// The loaded snapshot is missing; the file must be read again.
    NeedsReload,
    /// The file changed on disk since it was loaded. `Editor::external_change`
    /// describes the difference.
    ExternalChange(String),
    Failed(CoreError),
    /// The write finished (`written`), but reading the file back failed.
    /// The editor keeps the previously loaded state.
    Unverified {
        written: bool,
        error: CoreError,
    },
}

pub(crate) struct Editor {
    configs: VideoConfigService,
    path: Option<PathBuf>,
    original: Option<VideoConfigState>,
    snapshot: Option<VideoConfigSnapshot>,
    pub(crate) aspect_index: usize,
    preset_choices: Vec<PresetChoice>,
    selected_preset: usize,
    /// Text fields bound to the UI. Call `refresh` after changing them.
    pub(crate) custom_width: String,
    pub(crate) custom_height: String,
    supported_modes: HashSet<(i32, i32)>,
    pub(crate) create_backup: bool,
    // Derived from the fields above by `refresh`; the frame loop never
    // re-parses or re-formats these.
    edit: EditState,
    /// Last valid pending resolution, kept so the aspect preview stays stable
    /// while custom-dimension typing passes through intermediate invalid states.
    /// Never used for Apply; `edit` alone gates `can_apply()`.
    preview_fallback: Option<Resolution>,
    current_label: String,
    pending_label: String,
    validation: String,
    availability: Availability,
    external_change: Option<String>,
}

impl Editor {
    pub(crate) fn new(configs: VideoConfigService) -> Self {
        let initial = ResolutionCatalog::recommended_preset(0).expect("mode 0 preset");
        let mut editor = Self {
            configs,
            path: None,
            original: None,
            snapshot: None,
            aspect_index: 0,
            preset_choices: Vec::new(),
            selected_preset: 0,
            custom_width: initial.width.to_string(),
            custom_height: initial.height.to_string(),
            supported_modes: HashSet::new(),
            create_backup: true,
            edit: EditState::NoFile,
            preview_fallback: None,
            current_label: String::new(),
            pending_label: String::new(),
            validation: String::new(),
            availability: Availability::Unknown,
            external_change: None,
        };
        editor.populate_preset_choices(0, Some(initial.width), Some(initial.height));
        editor.refresh();
        editor
    }

    pub(crate) fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub(crate) fn original(&self) -> Option<VideoConfigState> {
        self.original
    }

    pub(crate) fn edit(&self) -> EditState {
        self.edit
    }

    pub(crate) fn preview_resolution(&self) -> Option<Resolution> {
        self.edit.resolution().or(self.preview_fallback)
    }

    pub(crate) fn preset_choices(&self) -> &[PresetChoice] {
        &self.preset_choices
    }

    pub(crate) fn selected_preset(&self) -> usize {
        self.selected_preset
    }

    pub(crate) fn current_label(&self) -> &str {
        &self.current_label
    }

    pub(crate) fn pending_label(&self) -> &str {
        &self.pending_label
    }

    pub(crate) fn validation(&self) -> &str {
        &self.validation
    }

    pub(crate) fn availability(&self) -> Availability {
        self.availability
    }

    pub(crate) fn external_change(&self) -> Option<&str> {
        self.external_change.as_deref()
    }

    /// Reads `path` and makes it the edited file. A failed read leaves the
    /// current file, snapshot, and pending edit untouched.
    pub(crate) fn load(&mut self, path: PathBuf) -> Result<(), CoreError> {
        let snapshot = self.configs.read_snapshot(&path)?;
        self.path = Some(path);
        self.original = Some(snapshot.state);
        self.snapshot = Some(snapshot);
        self.external_change = None;
        self.reset_pending();
        Ok(())
    }

    /// Forgets the edited file.
    pub(crate) fn clear(&mut self) {
        self.path = None;
        self.original = None;
        self.snapshot = None;
        self.external_change = None;
        self.refresh();
    }

    /// Replaces the pending values with the loaded ones.
    pub(crate) fn reset_pending(&mut self) {
        let Some(original) = self.original else {
            return;
        };
        self.aspect_index = original.aspect_mode.clamp(0, 2) as usize;
        self.populate_preset_choices(
            self.aspect_index as i32,
            Some(original.width),
            Some(original.height),
        );
        self.custom_width = original.width.to_string();
        self.custom_height = original.height.to_string();
        self.refresh();
    }

    pub(crate) fn select_preset(&mut self, index: usize) {
        self.selected_preset = index;
        if index == 0 {
            if let Some(original) = self.original {
                self.custom_width = original.width.to_string();
                self.custom_height = original.height.to_string();
            }
        } else if let Some(resolution) = self.preset_choices.get(index).and_then(|c| c.resolution) {
            self.custom_width = resolution.width.to_string();
            self.custom_height = resolution.height.to_string();
            self.aspect_index = resolution.mode.clamp(0, 2) as usize;
        }
        self.refresh();
    }

    /// Switches the aspect family and selects its recommended preset.
    pub(crate) fn select_aspect(&mut self, index: usize) {
        self.aspect_index = index;
        let Ok(recommended) = ResolutionCatalog::recommended_preset(index as i32) else {
            return;
        };
        self.populate_preset_choices(
            index as i32,
            Some(recommended.width),
            Some(recommended.height),
        );
        self.custom_width = recommended.width.to_string();
        self.custom_height = recommended.height.to_string();
        self.refresh();
    }

    /// Adopts the modes reported for the selected display.
    pub(crate) fn set_supported_modes(&mut self, modes: HashSet<(i32, i32)>) {
        self.supported_modes = modes;
        let width = self.custom_width.parse::<i32>().ok();
        let height = self.custom_height.parse::<i32>().ok();
        self.populate_preset_choices(self.aspect_index as i32, width, height);
        self.refresh();
    }

    pub(crate) fn populate_preset_choices(
        &mut self,
        mode: i32,
        width: Option<i32>,
        height: Option<i32>,
    ) {
        let mut displayed: Vec<Resolution> = self
            .supported_modes
            .iter()
            // Portrait modes (rotated monitors) would otherwise land in the
            // 4:3 family as their nearest ratio.
            .filter(|(w, h)| w >= h)
            .filter_map(|(w, h)| ResolutionCatalog::parse(&format!("{w}x{h}"), None).ok())
            .filter(|r| r.mode == mode)
            .collect();
        displayed.sort_by(|a, b| a.width.cmp(&b.width).then(a.height.cmp(&b.height)));
        for preset in ResolutionCatalog::presets()
            .iter()
            .filter(|p| p.mode == mode)
        {
            if !displayed
                .iter()
                .any(|r| r.width == preset.width && r.height == preset.height)
            {
                displayed.push(*preset);
            }
        }

        self.preset_choices.clear();
        self.preset_choices.push(PresetChoice {
            resolution: None,
            label: "Custom resolution".to_string(),
        });
        for resolution in displayed {
            let status = if self
                .supported_modes
                .contains(&(resolution.width, resolution.height))
            {
                "available"
            } else {
                "not reported"
            };
            self.preset_choices.push(PresetChoice {
                resolution: Some(resolution),
                label: format!(
                    "{}  ({})  —  {status}",
                    resolution.display(),
                    resolution.ratio
                ),
            });
        }
        self.selected_preset = match (width, height) {
            (Some(w), Some(h)) => self
                .preset_choices
                .iter()
                .position(|c| c.resolution.map(|r| (r.width, r.height)) == Some((w, h)))
                .unwrap_or(0),
            _ => 0,
        };
    }

    fn pending_resolution(&self) -> Result<Resolution, String> {
        ResolutionCatalog::parse(
            &format!("{}x{}", self.custom_width, self.custom_height),
            Some(self.aspect_index as i32),
        )
        .map_err(|e| e.to_string())
    }

    /// Recomputes the edit state and labels from the pending fields.
    pub(crate) fn refresh(&mut self) {
        let Some(original) = self.original else {
            self.edit = EditState::NoFile;
            self.preview_fallback = None;
            self.current_label = "—".to_string();
            self.pending_label = "—".to_string();
            self.validation.clear();
            self.availability = Availability::Unknown;
            return;
        };
        self.current_label = format!(
            "Current:  {} × {}  ·  {}",
            original.width,
            original.height,
            ResolutionCatalog::mode_name(original.aspect_mode, original.width, original.height)
                .unwrap_or("?")
        );
        match self.pending_resolution() {
            Ok(resolution) => {
                // Files without an aspect-mode entry only store dimensions,
                // so an aspect-only change has nothing to write.
                self.edit = if original.matches(&resolution) {
                    EditState::Unchanged { resolution }
                } else {
                    EditState::Changed { resolution }
                };
                self.preview_fallback = Some(resolution);
                self.pending_label = format!(
                    "Pending:  {} × {}  ·  {}",
                    resolution.width,
                    resolution.height,
                    ResolutionCatalog::mode_name(
                        resolution.mode,
                        resolution.width,
                        resolution.height
                    )
                    .unwrap_or("?")
                );
                self.validation.clear();
                self.availability = if self.supported_modes.is_empty() {
                    Availability::NoModes
                } else if self
                    .supported_modes
                    .contains(&(resolution.width, resolution.height))
                {
                    Availability::Reported
                } else {
                    Availability::NotReported
                };
            }
            Err(message) => {
                self.edit = EditState::Invalid;
                self.pending_label = "Pending: invalid custom resolution".to_string();
                self.validation = message;
                self.availability = Availability::Unknown;
            }
        }
    }

    pub(crate) fn has_pending_changes(&self) -> bool {
        // Invalid custom dimensions keep Reset enabled as a way back.
        !matches!(self.edit, EditState::NoFile | EditState::Unchanged { .. })
    }

    pub(crate) fn can_apply(&self) -> bool {
        matches!(self.edit, EditState::Changed { .. })
    }

    /// Short always-visible reason why Apply is disabled; `None` when Apply
    /// is available. Complements the hover tooltip, which touch and keyboard
    /// users can never see.
    pub(crate) fn apply_blocker_reason(&self) -> Option<&'static str> {
        match self.edit {
            EditState::Changed { .. } => None,
            EditState::NoFile => Some("Select a configuration"),
            EditState::Invalid => Some("Enter valid dimensions"),
            EditState::Unchanged { .. } => Some("No changes to apply"),
        }
    }

    /// Writes the pending resolution if the file still matches the loaded
    /// snapshot, then reads it back.
    pub(crate) fn apply(&mut self) -> Result<ApplyOutcome, WriteError> {
        let (Some(path), EditState::Changed { resolution }) = (self.path.clone(), self.edit) else {
            return Err(WriteError::Unavailable);
        };
        let Some(snapshot) = self.snapshot.as_ref() else {
            return Err(WriteError::NeedsReload);
        };
        let UpdateResult {
            changed,
            backup_path,
        } = match self
            .configs
            .update_if_unchanged(&path, &resolution, self.create_backup, snapshot)
        {
            Ok(result) => result,
            Err(err) => return Err(self.write_failure(&path, err)),
        };
        self.load(path).map_err(|error| WriteError::Unverified {
            written: changed,
            error,
        })?;
        Ok(if changed {
            ApplyOutcome::Applied {
                backup: backup_path.map(PathBuf::from),
            }
        } else {
            ApplyOutcome::AlreadyCurrent
        })
    }

    /// Recognized backups of the edited file, newest first.
    pub(crate) fn backups(&self) -> Result<Vec<BackupInfo>, CoreError> {
        match &self.path {
            Some(path) => self.configs.get_backups(path),
            None => Ok(Vec::new()),
        }
    }

    /// Restores `backup` if the file still matches the loaded snapshot, then
    /// reloads the file and discards pending edits.
    pub(crate) fn restore(&mut self, backup: &Path) -> Result<(), WriteError> {
        let Some(path) = self.path.clone() else {
            return Err(WriteError::Unavailable);
        };
        let Some(snapshot) = self.snapshot.as_ref() else {
            return Err(WriteError::NeedsReload);
        };
        if let Err(err) = self.configs.restore_if_unchanged(&path, backup, snapshot) {
            return Err(self.write_failure(&path, err));
        }
        self.load(path).map_err(|error| WriteError::Unverified {
            written: true,
            error,
        })
    }

    fn write_failure(&mut self, path: &Path, err: CoreError) -> WriteError {
        match err {
            CoreError::ExternalChange(message) => {
                if let Some(loaded) = self.original {
                    self.external_change =
                        Some(describe_external_change(loaded, self.configs.read(path)));
                }
                WriteError::ExternalChange(message)
            }
            other => WriteError::Failed(other),
        }
    }

    /// Test hook: pretend `state` was loaded, without a file or snapshot.
    #[cfg(test)]
    pub(crate) fn set_loaded_state(&mut self, state: VideoConfigState) {
        self.original = Some(state);
    }

    /// Test hook: replace the pending label, to exercise long-text layout.
    #[cfg(test)]
    pub(crate) fn set_pending_label(&mut self, label: String) {
        self.pending_label = label;
    }

    /// Test hook: show `path` as the edited file without reading it.
    #[cfg(test)]
    pub(crate) fn set_path(&mut self, path: PathBuf) {
        self.path = Some(path);
    }
}

pub(crate) fn describe_external_change(
    loaded: VideoConfigState,
    current: Result<VideoConfigState, CoreError>,
) -> String {
    let loaded_label = describe_video_state(loaded);
    match current {
        Ok(current) if current == loaded => format!(
            "Loaded: {loaded_label}. On disk: display settings are the same; other file content changed. Reload before applying or restoring."
        ),
        Ok(current) => format!(
            "Loaded: {loaded_label}. On disk: {}. Reload before applying or restoring.",
            describe_video_state(current)
        ),
        Err(CoreError::Io(_)) => format!(
            "Loaded: {loaded_label}. The file on disk could not be read. Resolve the file access issue and reload."
        ),
        Err(_) => format!(
            "Loaded: {loaded_label}. The file on disk no longer has valid display settings. Resolve the file and reload."
        ),
    }
}

pub(crate) fn describe_video_state(state: VideoConfigState) -> String {
    let aspect = ResolutionCatalog::mode_name(state.aspect_mode, state.width, state.height)
        .unwrap_or("unknown aspect");
    format!("{} × {} ({aspect})", state.width, state.height)
}

#[cfg(test)]
mod tests;
