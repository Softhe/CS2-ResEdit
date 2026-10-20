//! Navigation between configuration files and the user-facing results of
//! editing operations.
//!
//! `Editor` owns the file state and its transitions. This module decides
//! which file to edit (accounts, Browse, reload), asks before discarding
//! edits or writing while CS2 runs, remembers selections in preferences, and
//! turns `Editor` outcomes into status messages.

use super::editor::{ApplyOutcome, WriteError};
use super::*;

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
        let prior_path = self.editor.path().map(Path::to_path_buf);
        let had_pending_changes = self.editor.has_pending_changes();
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
        // Load the newly selected account.
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
        if self.editor.path() == prior_path.as_deref() && !refresh_has_warning {
            if let Some((message, warning)) = prior_operation {
                self.set_operation_status(&message, warning);
            }
        }
    }

    /// Opens `path` for editing, or forgets the current file for `None`, and
    /// remembers the selection. A failed load keeps the current file.
    pub(super) fn load_path(&mut self, path: Option<PathBuf>) -> bool {
        let Some(path) = path else {
            self.editor.clear();
            self.pending_changed();
            return true;
        };
        if let Err(err) = self.editor.load(path.clone()) {
            self.set_status(&err.to_string(), true);
            return false;
        }
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
        self.pending_changed();
        true
    }

    /// Refreshes the status hint after the pending edit changed.
    pub(super) fn pending_changed(&mut self) {
        self.editor.refresh();
        // Never clobber a sticky operation warning (apply/restore failure,
        // external change, preferences warning) with a routine pending-state
        // refresh; background display changes and keystrokes both funnel
        // through here. Invalid input is reported in the settings card.
        if self.operation_status.is_none() && !self.status_warning {
            match self.editor.edit() {
                EditState::Changed { .. } => {
                    self.status = "Review the pending settings, then apply the change.".into();
                }
                EditState::Unchanged { .. } => self.status = "No pending changes.".into(),
                EditState::NoFile | EditState::Invalid => {}
            }
        }
    }

    pub(super) fn reset_pending(&mut self) {
        self.editor.reset_pending();
        self.pending_changed();
    }

    pub(super) fn on_preset_selected(&mut self, index: usize) {
        self.editor.select_preset(index);
        self.pending_changed();
    }

    pub(super) fn on_aspect_selected(&mut self, index: usize) {
        self.editor.select_aspect(index);
        self.pending_changed();
    }

    pub(super) fn display_changed(&mut self) {
        let modes = self
            .displays
            .get(self.selected_display)
            .map(|d| d.modes.iter().map(|m| (m.width, m.height)).collect())
            .unwrap_or_default();
        self.editor.set_supported_modes(modes);
        self.pending_changed();
    }

    pub(super) fn apply(&mut self) {
        match self.editor.apply() {
            Ok(ApplyOutcome::Applied {
                backup: Some(backup),
            }) => {
                let name = backup
                    .file_name()
                    .unwrap_or(backup.as_os_str())
                    .to_string_lossy()
                    .to_string();
                self.set_operation_status(&format!("Changes applied. Backup: {name}"), false);
            }
            Ok(ApplyOutcome::Applied { backup: None }) => {
                self.set_operation_status("Changes applied.", false);
            }
            Ok(ApplyOutcome::AlreadyCurrent) => {
                self.set_operation_status("The configuration already has these settings.", false);
            }
            Err(WriteError::Unavailable) => {}
            Err(WriteError::NeedsReload) => {
                self.set_operation_status(
                    "Reload the configuration before applying changes.",
                    true,
                );
            }
            Err(WriteError::ExternalChange(message)) => self.set_operation_status(&message, true),
            Err(WriteError::Failed(err)) => self.set_operation_status(&err.to_string(), true),
            Err(WriteError::Unverified { written, error }) => {
                let outcome = if written {
                    "Changes were written, but verifying"
                } else {
                    "Verifying"
                };
                self.set_operation_status(
                    &format!(
                        "{outcome} the updated configuration failed: {error}. Reload the file."
                    ),
                    true,
                );
            }
        }
        self.pending_changed();
    }

    pub(super) fn open_backups(&mut self) {
        self.confirm_restore = false;
        self.selected_backup = None;
        if self.editor.path().is_none() {
            self.set_status("Select a configuration first.", true);
            return;
        }
        self.backups = match self.editor.backups() {
            Ok(backups) => backups,
            Err(err) => {
                self.set_status(&format!("Could not load backups: {err}"), true);
                return;
            }
        };
        self.show_backups = true;
    }

    pub(super) fn restore_selected_backup(&mut self) {
        let Some(backup) = self.selected_backup.and_then(|i| self.backups.get(i)) else {
            return;
        };
        let backup = PathBuf::from(&backup.path);
        let result = self.editor.restore(&backup);
        if !matches!(
            result,
            Err(WriteError::Unavailable | WriteError::NeedsReload)
        ) {
            self.confirm_restore = false;
        }
        match result {
            Ok(()) => {
                self.show_backups = false;
                match self.preferences.warning.clone() {
                    Some(warning) => self.set_operation_status(
                        &format!("Backup restored successfully. {warning}"),
                        true,
                    ),
                    None => self.set_operation_status("Backup restored successfully.", false),
                }
            }
            Err(WriteError::Unavailable) => {}
            Err(WriteError::NeedsReload) => {
                self.set_operation_status(
                    "Reload the configuration before restoring a backup.",
                    true,
                );
            }
            Err(WriteError::ExternalChange(message)) => self.set_operation_status(&message, true),
            Err(WriteError::Failed(err)) => self.set_operation_status(&err.to_string(), true),
            Err(WriteError::Unverified { error, .. }) => {
                // The restore itself succeeded; close the window like a success.
                self.show_backups = false;
                self.set_operation_status(
                    &format!("Backup restored, but reloading the configuration failed: {error}"),
                    true,
                );
            }
        }
        self.pending_changed();
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
            // Surface the real validation failure, not a generic message.
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
        if self.editor.can_apply() {
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
        if self.editor.has_pending_changes() {
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
                self.load_path(self.editor.path().map(Path::to_path_buf));
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
