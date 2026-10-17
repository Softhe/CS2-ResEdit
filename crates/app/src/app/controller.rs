//! File navigation and editing controller for CS2 ResEdit.
//!
//! `ResEditApp` owns rendering, background scans, and diagnostics; this
//! module owns the selected path, the guarded snapshot, the pending edit,
//! and every transition between them (`select` / `reload` / `apply` /
//! `restore`). Rendering code calls into these operations and never mutates
//! file state directly. The transitions are exercised without constructing
//! the whole UI through the tests in `app.rs`.

use super::*;

/// Explicit edit lifecycle for the loaded configuration. Replaces the old
/// parallel `pending: Option<Resolution>` + `pending_differs: Option<bool>`
/// fields, whose "stay in sync" invariant was a recurring bug source:
/// `None` used to mean both "no file" and "invalid input".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EditState {
    /// No configuration loaded; nothing to compare against.
    NoFile,
    /// A configuration is loaded but the custom dimensions do not parse.
    /// Reset stays enabled (C# parity); Apply is blocked.
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

impl ResEditApp {
    pub(super) fn select_account(&mut self, index: usize) {
        let has_config = self.accounts[index].has_config;
        let path = PathBuf::from(&self.accounts[index].config_path);
        if has_config {
            let previous = self.selected_account.replace(index);
            if !self.load_path(Some(path)) {
                self.selected_account = previous;
            }
        } else {
            self.selected_account = Some(index);
            self.load_path(None);
            self.set_status(
                "This account does not have a CS2 video configuration.",
                true,
            );
        }
    }

    pub(super) fn adopt_accounts(&mut self, root_count: usize, discovered: Vec<SteamAccount>) {
        let prior_operation = self.operation_status.clone();
        let mut refresh_has_warning = false;
        let prior_path = self.selected_path.clone();
        let had_pending_changes = self.has_pending_changes();
        self.accounts.clear();
        self.discovered_root_count = root_count;
        self.accounts.extend(discovered);
        for path in self.preferences.recent_config_paths.clone() {
            if !Path::new(&path).is_file() {
                continue;
            }
            let already = self
                .accounts
                .iter()
                .any(|a| fsutil::paths_equal_ci(&a.config_path, &path));
            if !already {
                self.accounts.push(custom_account(Path::new(&path)));
            }
        }
        // Keep the active file visible in the selector even when a refresh
        // no longer discovers it. Its loaded snapshot may still be useful.
        if let Some(prior) = &prior_path {
            if !self.accounts.iter().any(|account| {
                fsutil::paths_equal_ci(&account.config_path, &prior.to_string_lossy())
            }) {
                self.accounts.push(custom_account(prior));
            }
        }
        let mut selection: Option<usize> = None;
        if let Some(prior) = &prior_path {
            selection = self
                .accounts
                .iter()
                .position(|a| fsutil::paths_equal_ci(&a.config_path, &prior.to_string_lossy()));
        }
        if selection.is_none() && prior_path.is_none() {
            if let Some(last_path) = &self.preferences.last_config_path {
                selection = self
                    .accounts
                    .iter()
                    .position(|a| fsutil::paths_equal_ci(&a.config_path, last_path));
            }
        }
        if selection.is_none() && prior_path.is_none() {
            if let Some(last) = &self.preferences.last_account_id.clone() {
                selection = self
                    .accounts
                    .iter()
                    .position(|a| a.account_id == *last && a.has_config);
            }
        }
        if selection.is_none() {
            selection = self.accounts.iter().position(|a| a.has_config);
        }
        self.selected_account = selection;
        if self.accounts.is_empty() {
            self.set_background_status(
                "No Steam accounts found. Use Browse to select cs2_video.txt.",
                false,
            );
        } else {
            self.set_background_status("Steam accounts refreshed.", false);
        }
        // Load the newly selected account (mirrors AccountChanged via selection).
        if let Some(index) = self.selected_account {
            let account = self.accounts[index].clone();
            if account.has_config {
                if had_pending_changes
                    && prior_path.as_ref().is_some_and(|prior| {
                        fsutil::paths_equal_ci(&account.config_path, &prior.to_string_lossy())
                    })
                {
                    self.set_background_status(
                        "Steam accounts refreshed. Unsaved changes kept.",
                        false,
                    );
                } else if !self.load_path(Some(PathBuf::from(&account.config_path))) {
                    refresh_has_warning = true;
                    self.selected_account = prior_path.as_ref().and_then(|prior| {
                        self.accounts.iter().position(|entry| {
                            fsutil::paths_equal_ci(&entry.config_path, &prior.to_string_lossy())
                        })
                    });
                } else {
                    // `load_path` reports "Configuration loaded."; preserve the
                    // refresh confirmation that triggered the load.
                    let loaded = self.status.clone();
                    let warning = self.status_warning;
                    refresh_has_warning = warning;
                    let combined = if loaded.starts_with("Configuration loaded") {
                        loaded.replacen(
                            "Configuration loaded",
                            "Steam accounts refreshed. Configuration loaded",
                            1,
                        )
                    } else {
                        format!("Steam accounts refreshed. {loaded}")
                    };
                    self.set_background_status(&combined, warning);
                }
            } else {
                self.load_path(None);
                self.set_background_status(
                    "This account does not have a CS2 video configuration.",
                    true,
                );
            }
        } else {
            self.load_path(None);
        }
        // A new reload error or preference warning takes precedence over an
        // earlier operation result, even when the old snapshot remains loaded.
        if self.selected_path == prior_path && !refresh_has_warning {
            if let Some((message, warning)) = prior_operation {
                self.set_operation_status(&message, warning);
            }
        }
    }

    pub(super) fn load_path(&mut self, path: Option<PathBuf>) -> bool {
        let Some(path) = path else {
            self.selected_path = None;
            self.original = None;
            self.snapshot = None;
            self.external_change_notice = None;
            self.pending_changed();
            return true;
        };
        match self.configs.read_snapshot(&path) {
            Ok(snapshot) => {
                self.selected_path = Some(path.clone());
                self.original = None;
                self.snapshot = None;
                self.external_change_notice = None;
                self.original = Some(snapshot.state);
                self.snapshot = Some(snapshot);
                self.reset_pending();
                let account_id = self
                    .selected_account
                    .and_then(|i| self.accounts.get(i))
                    .map(|a| a.account_id.clone())
                    .unwrap_or_default();
                let recent = self.preferences_service().add_recent(
                    &self.preferences.recent_config_paths,
                    &path.to_string_lossy(),
                );
                self.preferences = self.preferences_service().save_selection_with_theme(
                    Some(&account_id),
                    Some(&path),
                    &recent,
                    self.preferences.theme,
                    self.settings_path_opt(),
                );
                if let Some(warning) = self.preferences.warning.clone() {
                    // A save failure supersedes the startup read warning.
                    self.load_warning = None;
                    self.set_status(&format!("Configuration loaded. {warning}"), true);
                } else if let Some(read_warning) = self.load_warning.clone() {
                    // Surface the startup read warning once instead of
                    // silently discarding it via the successful save.
                    self.load_warning = None;
                    self.set_status(&format!("Configuration loaded. {read_warning}"), true);
                } else {
                    self.set_status("Configuration loaded.", false);
                }
                true
            }
            Err(err) => {
                self.set_status(&err.to_string(), true);
                false
            }
        }
    }

    pub(super) fn reset_pending(&mut self) {
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
        self.pending_changed();
    }

    pub(super) fn on_preset_selected(&mut self, index: usize) {
        self.selected_preset = index;
        if index == 0 {
            if let Some(original) = self.original {
                self.custom_width = original.width.to_string();
                self.custom_height = original.height.to_string();
            }
            self.pending_changed();
            return;
        }
        let Some(choice) = self.preset_choices.get(index).cloned() else {
            self.pending_changed();
            return;
        };
        if let Some(resolution) = choice.resolution {
            self.custom_width = resolution.width.to_string();
            self.custom_height = resolution.height.to_string();
            self.aspect_index = resolution.mode.clamp(0, 2) as usize;
        }
        self.pending_changed();
    }

    pub(super) fn on_aspect_selected(&mut self, index: usize) {
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
        self.pending_changed();
    }

    pub(super) fn display_changed(&mut self) {
        self.supported_modes = self
            .displays
            .get(self.selected_display)
            .map(|d| d.modes.iter().map(|m| (m.width, m.height)).collect())
            .unwrap_or_default();
        let width = self.custom_width.parse::<i32>().ok();
        let height = self.custom_height.parse::<i32>().ok();
        self.populate_preset_choices(self.aspect_index as i32, width, height);
        self.pending_changed();
    }

    pub(super) fn populate_preset_choices(
        &mut self,
        mode: i32,
        width: Option<i32>,
        height: Option<i32>,
    ) {
        let mut supported: Vec<Resolution> = self
            .supported_modes
            .iter()
            // Portrait modes (rotated monitors) would otherwise land in the
            // 4:3 family as their nearest ratio.
            .filter(|(w, h)| w >= h)
            .filter_map(|(w, h)| ResolutionCatalog::parse(&format!("{w}x{h}"), None).ok())
            .filter(|r| r.mode == mode)
            .collect();
        supported.sort_by(|a, b| a.width.cmp(&b.width).then(a.height.cmp(&b.height)));
        let mut displayed: Vec<Resolution> = supported;
        for preset in ResolutionCatalog::presets()
            .iter()
            .filter(|p| p.mode == mode)
        {
            if !self
                .supported_modes
                .contains(&(preset.width, preset.height))
                && !displayed
                    .iter()
                    .any(|r| r.width == preset.width && r.height == preset.height)
            {
                displayed.push(*preset);
            }
        }
        // Deduplicate preserving order.
        let mut seen = HashSet::new();
        displayed.retain(|r| seen.insert((r.width, r.height)));

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
        let matching = (width, height);
        self.selected_preset = match matching {
            (Some(w), Some(h)) => self
                .preset_choices
                .iter()
                .position(|c| c.resolution.map(|r| (r.width, r.height)) == Some((w, h)))
                .unwrap_or(0),
            _ => 0,
        };
    }

    pub(super) fn pending_resolution(&self) -> Result<Resolution, String> {
        ResolutionCatalog::parse(
            &format!("{}x{}", self.custom_width, self.custom_height),
            Some(self.aspect_index as i32),
        )
        .map_err(|e| e.to_string())
    }

    pub(super) fn pending_changed(&mut self) {
        let Some(original) = self.original else {
            self.edit = EditState::NoFile;
            self.preview_fallback = None;
            self.current_label = "—".to_string();
            self.pending_label = "—".to_string();
            self.validation.clear();
            self.availability.clear();
            self.availability_ok = None;
            return;
        };
        self.current_label = {
            let name =
                ResolutionCatalog::mode_name(original.aspect_mode, original.width, original.height)
                    .unwrap_or("?");
            format!(
                "Current:  {} × {}  ·  {name}",
                original.width, original.height
            )
        };
        match self.pending_resolution() {
            Ok(resolution) => {
                // Files without an aspect-mode entry only store dimensions,
                // so an aspect-only change has nothing to write.
                let differs = !original.matches(&resolution);
                let name = ResolutionCatalog::mode_name(
                    resolution.mode,
                    resolution.width,
                    resolution.height,
                )
                .unwrap_or("?");
                self.edit = if differs {
                    EditState::Changed { resolution }
                } else {
                    EditState::Unchanged { resolution }
                };
                self.preview_fallback = Some(resolution);
                self.pending_label = format!(
                    "Pending:  {} × {}  ·  {name}",
                    resolution.width, resolution.height
                );
                self.validation.clear();
                if self.supported_modes.is_empty() {
                    self.availability = "Display modes unavailable".to_string();
                    self.availability_ok = None;
                } else if self
                    .supported_modes
                    .contains(&(resolution.width, resolution.height))
                {
                    // U+2714 is bundled; U+2713 is tofu.
                    self.availability = "✔ Reported for selected display".to_string();
                    self.availability_ok = Some(true);
                } else {
                    self.availability = "Not reported; custom use is still allowed".to_string();
                    self.availability_ok = Some(false);
                }
                // Never clobber a sticky operation warning (apply/restore
                // failure, external change, preferences warning) with a
                // routine pending-state refresh; background display changes
                // and keystrokes both funnel through here.
                if self.operation_status.is_none() && !self.status_warning {
                    self.status = if differs {
                        "Review the pending settings, then apply the change.".to_string()
                    } else {
                        "No pending changes.".to_string()
                    };
                    self.status_warning = false;
                }
            }
            Err(message) => {
                // Form feedback lives in the settings card (`validation` row
                // and per-field errors). The footer operation status is sticky
                // and must survive intermediate invalid keystrokes.
                self.edit = EditState::Invalid;
                self.pending_label = "Pending: invalid custom resolution".to_string();
                self.validation = message;
                self.availability.clear();
                self.availability_ok = None;
            }
        }
    }

    pub(super) fn has_pending_changes(&self) -> bool {
        // Invalid custom dimensions keep Reset enabled (C# parity).
        !matches!(self.edit, EditState::NoFile | EditState::Unchanged { .. })
    }

    pub(super) fn can_apply(&self) -> bool {
        matches!(self.edit, EditState::Changed { .. })
    }

    /// Short always-visible reason why Apply is disabled; `None` when Apply
    /// is available. Complements the hover tooltip, which touch and keyboard
    /// users can never see.
    pub(super) fn apply_blocker_reason(&self) -> Option<&'static str> {
        if self.can_apply() {
            return None;
        }
        match self.edit {
            EditState::NoFile => Some("Select a configuration"),
            EditState::Invalid => Some("Enter valid dimensions"),
            EditState::Unchanged { .. } | EditState::Changed { .. } => Some("No changes to apply"),
        }
    }

    pub(super) fn apply(&mut self) {
        let Some(path) = self.selected_path.clone() else {
            return;
        };
        let Some(resolution) = self.edit.resolution() else {
            return;
        };
        let Some(snapshot) = self.snapshot.as_ref() else {
            self.set_operation_status("Reload the configuration before applying changes.", true);
            return;
        };
        match self
            .configs
            .update_if_unchanged(&path, &resolution, self.create_backup, snapshot)
        {
            Ok(UpdateResult {
                changed,
                backup_path,
            }) => {
                match self.configs.read_snapshot(&path) {
                    Ok(snapshot) => {
                        self.original = Some(snapshot.state);
                        self.snapshot = Some(snapshot);
                        self.reset_pending();
                    }
                    Err(err) => {
                        let outcome = if changed {
                            "Changes were written, but verifying"
                        } else {
                            "Verifying"
                        };
                        self.set_operation_status(
                            &format!(
                                "{outcome} the updated configuration failed: {err}. Reload the file."
                            ),
                            true,
                        );
                        return;
                    }
                }
                if changed {
                    match backup_path {
                        Some(backup) => {
                            let name = Path::new(&backup)
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or(backup);
                            self.set_operation_status(
                                &format!("Changes applied. Backup: {name}"),
                                false,
                            );
                        }
                        None => self.set_operation_status("Changes applied.", false),
                    }
                } else {
                    self.set_operation_status(
                        "The configuration already has these settings.",
                        false,
                    );
                }
            }
            Err(CoreError::ExternalChange(message)) => {
                self.external_change_notice = Some(describe_external_change(
                    snapshot.state,
                    self.configs.read(&path),
                ));
                self.set_operation_status(&message, true);
            }
            Err(err) => self.set_operation_status(&err.to_string(), true),
        }
    }

    pub(super) fn open_backups(&mut self) {
        self.confirm_restore = false;
        self.selected_backup = None;
        let Some(path) = self.selected_path.as_ref() else {
            self.set_status("Select a configuration first.", true);
            return;
        };
        self.backups = match self.configs.get_backups(path) {
            Ok(backups) => backups,
            Err(err) => {
                self.set_status(&format!("Could not load backups: {err}"), true);
                return;
            }
        };
        self.show_backups = true;
    }

    pub(super) fn restore_selected_backup(&mut self) {
        let (Some(config), Some(index)) = (self.selected_path.clone(), self.selected_backup) else {
            return;
        };
        let Some(backup) = self.backups.get(index).cloned() else {
            return;
        };
        let Some(snapshot) = self.snapshot.as_ref() else {
            self.set_operation_status("Reload the configuration before restoring a backup.", true);
            return;
        };
        match self
            .configs
            .restore_if_unchanged(&config, Path::new(&backup.path), snapshot)
        {
            Ok(_) => {
                self.show_backups = false;
                self.confirm_restore = false;
                // A failed reload keeps the pre-restore state loaded, so the
                // return value (not `original`) tells whether it worked.
                if !self.load_path(Some(config)) {
                    let message = format!(
                        "Backup restored, but reloading the configuration failed: {}",
                        self.status
                    );
                    self.set_operation_status(&message, true);
                } else {
                    if let Some(warning) = self.preferences.warning.clone() {
                        self.set_operation_status(
                            &format!("Backup restored successfully. {warning}"),
                            true,
                        );
                    } else {
                        self.set_operation_status("Backup restored successfully.", false);
                    }
                }
            }
            Err(CoreError::ExternalChange(message)) => {
                self.external_change_notice = Some(describe_external_change(
                    snapshot.state,
                    self.configs.read(&config),
                ));
                self.confirm_restore = false;
                self.set_operation_status(&message, true);
            }
            Err(err) => {
                self.confirm_restore = false;
                self.set_operation_status(&err.to_string(), true);
            }
        }
    }

    /// Lets the user pick a configuration. A valid pick is opened, after
    /// confirmation when pending edits would be discarded.
    pub(super) fn browse(&mut self) {
        let Some(file) = rfd::FileDialog::new()
            .set_title("Select the CS2 video configuration")
            .add_filter("CS2 video configuration (cs2_video.txt)", &["txt"])
            .add_filter("All files", &["*"])
            .set_file_name("cs2_video.txt")
            .pick_file()
        else {
            return;
        };
        if let Err(err) = self.configs.read(&file) {
            // Surface the real validation failure (C# showed ex.Message).
            self.set_status(&err.to_string(), true);
            return;
        }
        self.request_discard(DiscardAction::OpenFile(file));
    }

    pub(super) fn open_file(&mut self, file: PathBuf) {
        let recent = self.preferences_service().add_recent(
            &self.preferences.recent_config_paths,
            &file.to_string_lossy(),
        );
        self.preferences.recent_config_paths = recent;
        if !self
            .accounts
            .iter()
            .any(|a| fsutil::paths_equal_ci(&a.config_path, &file.to_string_lossy()))
        {
            self.accounts.push(custom_account(&file));
        }
        self.selected_account = self
            .accounts
            .iter()
            .position(|a| fsutil::paths_equal_ci(&a.config_path, &file.to_string_lossy()));
        self.load_path(Some(file));
    }

    /// Applies the pending edit, asking first when CS2 is running.
    pub(super) fn request_apply(&mut self) {
        if self.can_apply() {
            self.guard_write(GuardedWrite::Apply);
        }
    }

    /// Restores the selected backup, asking first when CS2 is running.
    pub(super) fn request_restore(&mut self) {
        if self.selected_backup.is_some() {
            self.guard_write(GuardedWrite::Restore);
        }
    }

    fn guard_write(&mut self, write: GuardedWrite) {
        // An unreadable process list is not a reason to block the write.
        if (self.game_probe)() == Some(true) {
            self.confirmation = Some(Confirmation::GameRunning(write));
        } else {
            self.run_write(write);
        }
    }

    fn run_write(&mut self, write: GuardedWrite) {
        match write {
            GuardedWrite::Apply => self.apply(),
            GuardedWrite::Restore => self.restore_selected_backup(),
        }
    }

    /// Runs `action`, asking first when it would discard pending edits.
    pub(super) fn request_discard(&mut self, action: DiscardAction) {
        if matches!(action, DiscardAction::SelectAccount(index) if self.selected_account == Some(index))
        {
            // Re-selecting the active entry must not silently reload it.
            return;
        }
        if self.has_pending_changes() {
            let queued = match action {
                DiscardAction::SelectAccount(index) => match self.accounts.get(index) {
                    Some(account) => DiscardAction::SelectConfig(account.config_path.clone()),
                    None => return,
                },
                other => other,
            };
            self.confirmation = Some(Confirmation::DiscardEdits(queued));
        } else {
            self.run_discard(action);
        }
    }

    fn run_discard(&mut self, action: DiscardAction) {
        match action {
            DiscardAction::SelectAccount(index) if index < self.accounts.len() => {
                self.select_account(index);
            }
            DiscardAction::SelectAccount(_) => {}
            DiscardAction::SelectConfig(config_path) => {
                match self
                    .accounts
                    .iter()
                    .position(|a| fsutil::paths_equal_ci(&a.config_path, &config_path))
                {
                    Some(index) => self.select_account(index),
                    None => self.set_status(
                        "The selected account is no longer listed. Choose it again.",
                        true,
                    ),
                }
            }
            DiscardAction::Reload => {
                self.load_path(self.selected_path.clone());
            }
            DiscardAction::OpenFile(file) => self.open_file(file),
        }
    }

    /// Carries out the action behind the open prompt.
    pub(super) fn confirm(&mut self) {
        match self.confirmation.take() {
            Some(Confirmation::DiscardEdits(action)) => self.run_discard(action),
            Some(Confirmation::GameRunning(write)) => self.run_write(write),
            None => {}
        }
    }

    pub(super) fn cancel_confirmation(&mut self) {
        self.confirmation = None;
    }
}
