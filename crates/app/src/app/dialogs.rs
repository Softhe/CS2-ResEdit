//! Header, footer, and dialogs drawn around the main cards by
//! `ResEditApp::update`. Keeping them out of `update` lets rustfmt format
//! them and keeps the frame loop readable.

use super::*;

impl ResEditApp {
    /// Title, theme selector, and file sync badge.
    pub(super) fn header_panel(&mut self, ctx: &egui::Context, modal_open: bool) {
        let palette = self.palette;
        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.heading("CS2 ResEdit");
                    ui.label("Tune your display settings safely");
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let previous = self.preferences.theme;
                    if palette.high_contrast {
                        ui.label("High Contrast")
                            .on_hover_text("Theme follows Windows High Contrast settings");
                    } else {
                        ui.add_enabled_ui(!modal_open, |ui| {
                            let response = egui::ComboBox::from_id_salt("ui_theme")
                                .selected_text(self.preferences.theme.label())
                                .width(105.0)
                                .show_ui(ui, |ui| {
                                    for theme in UiTheme::ALL {
                                        ui.selectable_value(
                                            &mut self.preferences.theme,
                                            theme,
                                            theme.label(),
                                        );
                                    }
                                });
                            response.response.clone().labelled_by(ui.label("Theme").id);
                            // Force an accessible name like `named_combo` does for
                            // settings-card combos (egui 0.32 + Windows UIA).
                            let theme_enabled = ui.is_enabled();
                            response.response.widget_info(|| {
                                egui::WidgetInfo::labeled(
                                    egui::WidgetType::ComboBox,
                                    theme_enabled,
                                    "Theme",
                                )
                            });
                            let current = UiTheme::ALL
                                .iter()
                                .position(|theme| *theme == self.preferences.theme)
                                .unwrap_or(0);
                            if let Some(index) = panels::wheel_selection(
                                &response.response,
                                current,
                                UiTheme::ALL.len(),
                            ) {
                                self.preferences.theme = UiTheme::ALL[index];
                            }
                        });
                    }
                    if previous != self.preferences.theme {
                        self.palette = ThemePalette::for_theme(self.preferences.theme);
                        self.theme_applied = false;
                        self.preferences = self.preferences_service().save_selection_with_theme(
                            self.preferences.last_account_id.as_deref(),
                            self.preferences.last_config_path.as_deref().map(Path::new),
                            &self.preferences.recent_config_paths,
                            self.preferences.theme,
                            self.settings_path_opt(),
                        );
                        if let Some(warning) = self.preferences.warning.clone() {
                            self.set_status(&warning, true);
                        }
                        ctx.request_repaint();
                    }
                    // Header badge doubles as sync state: file name plus
                    // unsaved/synced marker instead of a static label.
                    let (badge, badge_color) = match &self.selected_path {
                        None => ("No configuration".to_string(), palette.muted),
                        Some(path) => {
                            let name = path
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_else(|| "cs2_video.txt".to_string());
                            let marker = if self.has_pending_changes() {
                                "• "
                            } else {
                                ""
                            };
                            (
                                format!("{marker}{name}"),
                                if self.has_pending_changes() {
                                    palette.accent
                                } else {
                                    palette.muted
                                },
                            )
                        }
                    };
                    ui.label(egui::RichText::new(badge).color(badge_color).strong())
                        .on_hover_text(match &self.selected_path {
                            None => "Select an account or browse to a config file".to_string(),
                            Some(path) => path.to_string_lossy().to_string(),
                        });
                });
            });
        });
    }

    /// Scan activity, status text, and the Reset / Apply actions.
    pub(super) fn footer_panel(&mut self, ctx: &egui::Context, modal_open: bool) {
        let palette = self.palette;
        egui::TopBottomPanel::bottom("footer").show(ctx, |ui| {
            ui.add_enabled_ui(!modal_open, |ui| {
                ui.horizontal(|ui| {
                    if let Some(activity) = self.scan_activity() {
                        ui.spinner();
                        ui.label(activity);
                    } else if let Some((notice, _)) = &self.scan_notice {
                        ui.label(notice);
                    }
                    if self.status_warning {
                        egui::Frame::new()
                            .fill(palette.accent_soft)
                            .corner_radius(egui::CornerRadius::same(6))
                            .inner_margin(8.0)
                            .show(ui, |ui| {
                                ui.label(
                                    egui::RichText::new(format!("⚠ {}", self.status))
                                        .color(palette.warning),
                                );
                            });
                    } else {
                        ui.label(egui::RichText::new(&self.status).color(palette.muted));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let apply =
                            ui.add_enabled(self.can_apply(), egui::Button::new("Apply changes"));
                        if apply.clicked() {
                            self.request_apply();
                        }
                        if !apply.enabled() {
                            apply.on_disabled_hover_text(if self.original.is_none() {
                                "Select a configuration first"
                            } else {
                                "Change the resolution or aspect to enable Apply"
                            });
                        }
                        let reset =
                            ui.add_enabled(self.has_pending_changes(), egui::Button::new("Reset"));
                        if reset.clicked() {
                            self.reset_pending();
                        }
                        if !reset.enabled() {
                            reset.on_disabled_hover_text(if self.original.is_none() {
                                "Nothing to reset yet"
                            } else {
                                "No pending changes to discard"
                            });
                        }
                        // Always-visible reason for a disabled Apply; hover
                        // tooltips alone exclude touch and keyboard users.
                        if let Some(reason) = self.apply_blocker_reason() {
                            ui.label(egui::RichText::new(reason).small().color(palette.muted));
                        }
                    });
                });
            });
        });
    }

    /// Backup list with restore confirmation.
    pub(super) fn backups_window(&mut self, ctx: &egui::Context) {
        if !self.show_backups {
            return;
        }
        let mut open = true;
        egui::Window::new("Configuration backups")
            .open(&mut open)
            .resizable(true)
            .default_size([560.0, 420.0])
            .min_size([420.0, 300.0])
            .show(ctx, |ui| {
                if let Some(comparison) = &self.external_change_notice {
                    ui.colored_label(self.palette.warning, comparison);
                    ui.separator();
                }
                if self.backups.is_empty() {
                    ui.label("No editor backups were found.");
                    ui.label(
                        "Apply a change with backups enabled to create one. \
                         Backups are stored next to the configuration.",
                    );
                    return;
                }
                self.backup_list(ui);
                self.selected_backup_details(ui);
                self.restore_actions(ui);
            });
        // A restore inside the window may already have closed it.
        self.show_backups &= open;
        if !open {
            self.confirm_restore = false;
        }
    }

    fn backup_list(&mut self, ui: &mut egui::Ui) {
        let mut clicked = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            for (index, backup) in self.backups.iter().enumerate() {
                let selected = self.selected_backup == Some(index);
                if ui
                    .selectable_label(selected, backup_label(backup))
                    .clicked()
                {
                    clicked = Some(index);
                }
            }
        });
        if let Some(index) = clicked {
            self.selected_backup = Some(index);
            self.confirm_restore = false;
        }
    }

    fn selected_backup_details(&self, ui: &mut egui::Ui) {
        let Some(backup) = self.selected_backup.and_then(|i| self.backups.get(i)) else {
            return;
        };
        ui.label(backup_label(backup));
        ui.add(egui::Label::new(egui::RichText::new(&backup.path).monospace().small()).truncate())
            .on_hover_text(&backup.path);
        match self.original {
            Some(original) if original != backup.state => {
                ui.label(format!(
                    "Current file: {} — restoring will overwrite these values.",
                    describe_video_state(original)
                ));
            }
            Some(_) => {
                ui.label("Backup matches the current file.");
            }
            None => {}
        }
    }

    fn restore_actions(&mut self, ui: &mut egui::Ui) {
        if !self.confirm_restore {
            let restore = egui::Button::new("Restore selected");
            if ui
                .add_enabled(self.selected_backup.is_some(), restore)
                .clicked()
            {
                self.confirm_restore = true;
            }
            return;
        }
        ui.label("Restore this backup? A rollback backup of the active file will be created.");
        ui.horizontal(|ui| {
            if ui.button("Confirm restore").clicked() {
                self.request_restore();
            }
            if ui.button("Cancel").clicked() {
                self.confirm_restore = false;
            }
        });
    }

    /// Privacy-safe diagnostics summary with copy and export actions.
    pub(super) fn diagnostics_window(&mut self, ctx: &egui::Context) {
        if self.show_diagnostics {
            let mut open = self.show_diagnostics;
            egui::Window::new("Privacy-safe diagnostics")
                .open(&mut open)
                .resizable(true)
                .default_size([640.0, 480.0])
                .min_size([480.0, 320.0])
                .show(ctx, |ui| {
                    ui.label("Counts and booleans only — no names, paths, or file contents.");
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::multiline(&mut self.diagnostics_summary.as_str())
                                .desired_width(f32::INFINITY)
                                .desired_rows(12)
                                .interactive(false),
                        );
                    });
                    ui.horizontal(|ui| {
                        if ui.button("Copy").clicked() {
                            ui.ctx().copy_text(self.diagnostics_summary.clone());
                        }
                        if ui.button("Export JSON").clicked() {
                            if let Some(path) = rfd::FileDialog::new()
                                .set_file_name("cs2-resedit-diagnostics.json")
                                .add_filter("JSON report (*.json)", &["json"])
                                .save_file()
                            {
                                match export_diagnostics(&path, &self.diagnostics_json) {
                                    Ok(()) => self.set_status(
                                        &format!("Diagnostics exported to {}.", path.display()),
                                        false,
                                    ),
                                    Err(err) => self.set_status(
                                        &format!("Diagnostics export failed: {err}"),
                                        true,
                                    ),
                                }
                            }
                        }
                    });
                });
            self.show_diagnostics = open;
        }
    }

    /// Modal prompt for the pending `confirmation`. Esc or a click outside
    /// cancels it.
    pub(super) fn confirmation_prompt(&mut self, ctx: &egui::Context) {
        let Some(confirmation) = self.confirmation.clone() else {
            return;
        };
        let (title, body, confirm_label, cancel_label) = match &confirmation {
            Confirmation::DiscardEdits(action) => (
                "Discard unsaved changes?",
                match action {
                    DiscardAction::SelectAccount(_) => {
                        "The pending resolution has not been applied. Switching accounts discards it."
                    }
                    DiscardAction::Reload => {
                        "The pending resolution has not been applied. Reloading the file discards it."
                    }
                    DiscardAction::OpenFile(_) => {
                        "The pending resolution has not been applied. Opening another file discards it."
                    }
                },
                "Discard changes",
                "Keep editing",
            ),
            Confirmation::GameRunning(write) => (
                "Counter-Strike 2 is running",
                "CS2 can overwrite cs2_video.txt with its in-memory settings when it exits. \
                 Close the game first, or continue anyway.",
                match write {
                    GuardedWrite::Apply => "Apply anyway",
                    GuardedWrite::Restore => "Restore anyway",
                },
                "Cancel",
            ),
        };
        let palette = self.palette;
        let mut choice = None;
        let response = egui::Modal::new(egui::Id::new("confirmation")).show(ctx, |ui| {
            ui.set_max_width(420.0);
            ui.heading(title);
            ui.add(egui::Label::new(body).wrap());
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui
                    .button(egui::RichText::new(confirm_label).color(palette.warning))
                    .clicked()
                {
                    choice = Some(true);
                }
                if ui.button(cancel_label).clicked() {
                    choice = Some(false);
                }
            });
        });
        match choice {
            Some(true) => self.confirm(),
            Some(false) => self.cancel_confirmation(),
            None if response.should_close() => self.cancel_confirmation(),
            None => {}
        }
    }
}

/// One-line backup summary: local timestamp, dimensions, and aspect family.
fn backup_label(backup: &cs2_resedit_core::BackupInfo) -> String {
    let created =
        chrono::DateTime::from_timestamp_nanos(backup.created).with_timezone(&chrono::Local);
    let state = backup.state;
    let mode =
        ResolutionCatalog::mode_name(state.aspect_mode, state.width, state.height).unwrap_or("?");
    format!(
        "{}  ·  {} × {}  ·  {mode}",
        created.format("%Y-%m-%d %H:%M:%S"),
        state.width,
        state.height
    )
}
