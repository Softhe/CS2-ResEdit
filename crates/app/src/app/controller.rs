//! Navigation between configuration files and the user-facing results of
//! editing operations.
//!
//! `Editor` owns the file state and its transitions. This module decides
//! which file to edit (accounts, Browse, reload), asks before discarding
//! edits or writing while CS2 runs, remembers selections in preferences, and
//! turns `Editor` outcomes into status notices.

use super::editor::{ApplyOutcome, WriteError};
use super::*;
use cs2_resedit_core::CoreError;

const NO_CONFIG: &str = "This account does not have a CS2 video configuration.";

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
            self.close_file();
            self.status.report(Notice::warning(NO_CONFIG));
        }
    }

    pub(super) fn adopt_accounts(&mut self, root_count: usize, discovered: Vec<SteamAccount>) {
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

        let refreshed = Notice::info(if self.accounts.is_empty() {
            "No Steam accounts found. Use Browse to select cs2_video.txt."
        } else {
            "Steam accounts refreshed."
        });
        let notice = match self
            .selected_account
            .map(|index| self.accounts[index].clone())
        {
            None => {
                self.close_file();
                refreshed
            }
            Some(account) if !account.has_config => {
                self.close_file();
                refreshed.then(Notice::warning(NO_CONFIG))
            }
            Some(account)
                if had_pending_changes
                    && prior_path.as_ref().is_some_and(|prior| {
                        fsutil::paths_equal_ci(&account.config_path, &prior.to_string_lossy())
                    }) =>
            {
                refreshed.then(Notice::info("Unsaved changes kept."))
            }
            Some(account) => match self.open_path(PathBuf::from(&account.config_path)) {
                Ok(()) => refreshed.then(Notice::info("Configuration loaded.")),
                Err(err) => {
                    self.selected_account = prior_path.as_ref().and_then(|prior| {
                        self.accounts.iter().position(|entry| {
                            fsutil::paths_equal_ci(&entry.config_path, &prior.to_string_lossy())
                        })
                    });
                    Notice::warning(err.to_string())
                }
            },
        };
        // An operation result describes the file it ran on. It stays through
        // a refresh of that file unless the refresh found a problem.
        if notice.warning || self.editor.path() != prior_path.as_deref() {
            self.status.report(notice);
        } else {
            self.status.report_background(notice);
        }
    }

    /// Opens `path` for editing and remembers the selection. A failed load
    /// keeps the current file. Reports nothing; callers choose the notice.
    fn open_path(&mut self, path: PathBuf) -> Result<(), CoreError> {
        self.editor.load(path.clone())?;
        self.preferences.last_account_id = self
            .selected_account
            .and_then(|i| self.accounts.get(i))
            .map(|a| a.account_id.clone())
            .filter(|id| !id.is_empty());
        self.preferences.last_config_path = Some(path.to_string_lossy().to_string());
        self.preferences.recent_config_paths = self.preferences_service().add_recent(
            &self.preferences.recent_config_paths,
            &path.to_string_lossy(),
        );
        self.save_preferences();
        self.pending_changed();
        Ok(())
    }

    fn close_file(&mut self) {
        self.editor.clear();
        self.pending_changed();
    }

    /// Opens `path` and reports the result, or closes the file for `None`.
    /// Returns whether the requested file is now loaded.
    pub(super) fn load_path(&mut self, path: Option<PathBuf>) -> bool {
        let Some(path) = path else {
            self.close_file();
            return true;
        };
        match self.open_path(path) {
            Ok(()) => {
                self.status.report(Notice::info("Configuration loaded."));
                true
            }
            Err(err) => {
                self.status.report(Notice::warning(err.to_string()));
                false
            }
        }
    }

    /// Refreshes the edit state and its status hint after the pending edit
    /// changed. Invalid input is reported in the settings card instead.
    pub(super) fn pending_changed(&mut self) {
        self.editor.refresh();
        match self.editor.edit() {
            EditState::Changed { .. } => self.status.hint(Notice::info(
                "Review the pending settings, then apply the change.",
            )),
            EditState::Unchanged { .. } => self.status.hint(Notice::info("No pending changes.")),
            EditState::NoFile | EditState::Invalid => {}
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
        let notice = match self.editor.apply() {
            Ok(ApplyOutcome::Applied {
                backup: Some(backup),
            }) => Notice::info(format!(
                "Changes applied. Backup: {}",
                backup
                    .file_name()
                    .unwrap_or(backup.as_os_str())
                    .to_string_lossy()
            )),
            Ok(ApplyOutcome::Applied { backup: None }) => Notice::info("Changes applied."),
            Ok(ApplyOutcome::AlreadyCurrent) => {
                Notice::info("The configuration already has these settings.")
            }
            Err(WriteError::Unavailable) => return,
            Err(WriteError::NeedsReload) => {
                Notice::warning("Reload the configuration before applying changes.")
            }
            Err(WriteError::Unverified { written, error }) => Notice::warning(format!(
                "{} the updated configuration failed: {error}. Reload the file.",
                if written {
                    "Changes were written, but verifying"
                } else {
                    "Verifying"
                }
            )),
            Err(err) => write_error_notice(err),
        };
        self.status.report_operation(notice);
        self.pending_changed();
    }

    pub(super) fn open_backups(&mut self) {
        self.confirm_restore = false;
        self.selected_backup = None;
        if self.editor.path().is_none() {
            self.status
                .report(Notice::warning("Select a configuration first."));
            return;
        }
        self.backups = match self.editor.backups() {
            Ok(backups) => backups,
            Err(err) => {
                self.status
                    .report(Notice::warning(format!("Could not load backups: {err}")));
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
        let notice = match self.editor.restore(&backup) {
            Ok(()) => {
                self.show_backups = false;
                Notice::info("Backup restored successfully.")
            }
            Err(WriteError::Unavailable) => return,
            Err(WriteError::NeedsReload) => {
                Notice::warning("Reload the configuration before restoring a backup.")
            }
            Err(WriteError::Unverified { error, .. }) => {
                // The restore itself succeeded; close the window like a success.
                self.show_backups = false;
                Notice::warning(format!(
                    "Backup restored, but reloading the configuration failed: {error}"
                ))
            }
            Err(err) => {
                self.confirm_restore = false;
                write_error_notice(err)
            }
        };
        self.confirm_restore &= self.show_backups;
        self.status.report_operation(notice);
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
            self.status.report(Notice::warning(err.to_string()));
            return;
        }
        self.request_discard(DiscardAction::OpenFile(file));
    }

    pub(super) fn open_file(&mut self, file: PathBuf) {
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
                    None => self.status.report(Notice::warning(
                        "The selected account is no longer listed. Choose it again.",
                    )),
                }
            }
            DiscardAction::Reload => {
                if let Some(path) = self.editor.path().map(Path::to_path_buf) {
                    self.load_path(Some(path));
                }
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

/// Notice for a failed write that the operation itself does not explain.
fn write_error_notice(err: WriteError) -> Notice {
    match err {
        WriteError::ExternalChange(message) => Notice::warning(message),
        WriteError::Failed(err) => Notice::warning(err.to_string()),
        WriteError::Unverified { error, .. } => Notice::warning(error.to_string()),
        WriteError::Unavailable | WriteError::NeedsReload => {
            Notice::warning("The configuration is not ready to write. Reload it.")
        }
    }
}
