//! Desktop UI and application state for CS2 ResEdit.
//!
//! The layout has a header, a settings card and a
//! preview card (side by side in the fixed-size window), and a footer
//! with status text plus Reset / Apply actions.
//!
//! `ResEditApp` owns discovery, dialogs, theming, and preferences. The file
//! being edited lives in `editor::Editor`, which has no UI dependency.

use cs2_resedit_core::{
    fsutil, DiagnosticService, DisplayInfo, DisplayModeProvider, DisplayProvider, Preferences,
    PreferencesService, ResolutionCatalog, SteamAccount, SteamService, UiTheme, VideoConfigService,
};
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::{Duration, Instant};

use crate::theme::ThemePalette;

mod controller;
mod dialogs;
mod editor;
mod panels;
mod status;
#[cfg(test)]
use panels::{preview_rectangle, preview_viewport};

use editor::{describe_video_state, Availability, EditState, Editor};
use status::{Notice, StatusLine};

const ASPECT_LABELS: [&str; 3] = ["4:3 / 5:4", "16:9", "16:10"];
const DISPLAY_POLL_INTERVAL: Duration = Duration::from_secs(2);
const SLOW_SCAN_THRESHOLD: Duration = Duration::from_millis(150);
const SCAN_NOTICE_DURATION: Duration = Duration::from_secs(4);
const SCAN_TIMEOUT: Duration = Duration::from_secs(10);
/// Quiet period before preference changes are written, so a burst of
/// selections (wheel scrolling through themes, rapid account switches)
/// becomes one write.
const PREFERENCES_SAVE_DELAY: Duration = Duration::from_millis(500);
/// Passes per frame: one to draw, then corrections while the equal-height
/// cards settle after content or style changes (see `show_main_cards`).
const CARD_PASSES: std::num::NonZeroUsize = std::num::NonZeroUsize::new(3).unwrap();

/// Background enumeration request. Only the newest request is polled; a
/// timed-out request it replaced moves to the retired list, whose late
/// results are discarded unread (see `prepare_scan`).
struct PendingScan<T> {
    receiver: mpsc::Receiver<T>,
    started: Instant,
    timeout_reported: bool,
}

/// An action that waits for the user to confirm it in a modal prompt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Confirmation {
    /// Pending edits would be lost by switching to another file state.
    DiscardEdits(DiscardAction),
    /// CS2 is running and may overwrite the file when it exits.
    GameRunning(GuardedWrite),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DiscardAction {
    /// An account picked in the selector, by its current list position.
    /// Prompts store `SelectConfig` instead.
    SelectAccount(usize),
    /// A queued account switch, identified by its configuration path: an
    /// account scan may rebuild and reorder the list while the prompt is open.
    SelectConfig(String),
    Reload,
    /// A file the user picked and that already passed validation.
    OpenFile(PathBuf),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GuardedWrite {
    Apply,
    Restore,
}

// Keep at most two workers per scan type, including abandoned workers.
// A blocked OS call cannot be cancelled safely; retain its receiver until exit.
fn prepare_scan<T>(
    active: &mut Option<PendingScan<T>>,
    retired: &mut Vec<mpsc::Receiver<T>>,
) -> bool {
    retired.retain(|receiver| matches!(receiver.try_recv(), Err(mpsc::TryRecvError::Empty)));
    if active.as_ref().is_some_and(|scan| !scan.timeout_reported)
        || retired.len() + usize::from(active.is_some()) >= 2
    {
        return false;
    }
    if let Some(scan) = active.take() {
        retired.push(scan.receiver);
    }
    true
}

type AccountScanResult = (usize, Vec<SteamAccount>, Vec<String>);

pub struct ResEditApp {
    configs: VideoConfigService,
    display_provider: Arc<dyn DisplayModeProvider>,
    settings_path: Option<PathBuf>,
    steam_root: Option<String>,
    preferences: Preferences,
    palette: ThemePalette,

    accounts: Vec<SteamAccount>,
    selected_account: Option<usize>,
    displays: Vec<DisplayInfo>,
    selected_display: usize,

    /// The configuration being edited and its pending values.
    editor: Editor,
    /// Footer message and its precedence rules.
    status: StatusLine,

    show_backups: bool,
    backups: Vec<cs2_resedit_core::BackupInfo>,
    selected_backup: Option<usize>,
    confirm_restore: bool,
    show_diagnostics: bool,
    diagnostics_summary: String,
    diagnostics_json: String,
    discovered_root_count: usize,
    display_labels: Vec<String>,
    initialized: bool,
    theme_applied: bool,
    #[cfg(windows)]
    taskbar_icon: crate::taskbar_icon::TaskbarIcon,
    last_display_poll: Instant,
    account_scan: Option<PendingScan<AccountScanResult>>,
    display_scan: Option<PendingScan<Vec<DisplayInfo>>>,
    scan_notice: Option<(String, Instant)>,
    retired_account_scans: Vec<mpsc::Receiver<AccountScanResult>>,
    retired_display_scans: Vec<mpsc::Receiver<Vec<DisplayInfo>>>,
    /// Shared card height from the previous pass (see `show_main_cards`).
    card_height: f32,
    /// When unsaved preference changes are due to be written.
    preferences_save_due: Option<Instant>,
    /// Prompt waiting for the user before a destructive or risky action.
    confirmation: Option<Confirmation>,
    /// Reports whether CS2 is running (`None` when unknown). Replaced in tests.
    game_probe: fn() -> Option<bool>,
    /// Reports Windows High Contrast. Replaced in tests.
    high_contrast_probe: fn() -> bool,
    /// `CS2_RESEDIT_HIGH_CONTRAST` forces the high-contrast palette.
    high_contrast_forced: bool,
}

impl ResEditApp {
    pub fn new(
        settings_path: Option<PathBuf>,
        steam_root: Option<String>,
        display_provider: Option<Box<dyn DisplayModeProvider>>,
    ) -> Self {
        let high_contrast_forced =
            std::env::var("CS2_RESEDIT_HIGH_CONTRAST").is_ok_and(|v| !v.is_empty() && v != "0");
        let high_contrast = high_contrast_forced || crate::system::high_contrast_enabled();
        Self {
            configs: VideoConfigService,
            display_provider: Arc::from(
                display_provider.unwrap_or_else(|| Box::new(DisplayProvider)),
            ),
            settings_path,
            steam_root,
            preferences: Preferences::default(),
            palette: ThemePalette::current(high_contrast),
            accounts: Vec::new(),
            selected_account: None,
            displays: Vec::new(),
            selected_display: 0,
            editor: Editor::new(VideoConfigService),
            status: StatusLine::default(),
            show_backups: false,
            backups: Vec::new(),
            selected_backup: None,
            confirm_restore: false,
            show_diagnostics: false,
            diagnostics_summary: String::new(),
            diagnostics_json: String::new(),
            discovered_root_count: 0,
            display_labels: Vec::new(),
            initialized: false,
            theme_applied: false,
            #[cfg(windows)]
            taskbar_icon: crate::taskbar_icon::TaskbarIcon::default(),
            last_display_poll: Instant::now(),
            account_scan: None,
            display_scan: None,
            scan_notice: None,
            retired_account_scans: Vec::new(),
            retired_display_scans: Vec::new(),
            card_height: 0.0,
            preferences_save_due: None,
            confirmation: None,
            game_probe: crate::system::game_running,
            high_contrast_probe: crate::system::high_contrast_enabled,
            high_contrast_forced,
        }
    }

    fn preferences_service(&self) -> PreferencesService {
        PreferencesService::new(self.configs)
    }

    fn settings_path_opt(&self) -> Option<&Path> {
        self.settings_path.as_deref()
    }

    fn initialize_data(&mut self) {
        let service = self.preferences_service();
        self.preferences = service.read(self.settings_path_opt());
        if !self.palette.high_contrast {
            self.palette = ThemePalette::for_theme(self.preferences.theme);
            self.theme_applied = false;
        }
        if let Some(warning) = self.preferences.warning.take() {
            self.status.preferences_read_failed(warning);
        }
        self.refresh_displays();
        self.refresh_accounts();
        self.initialized = true;
    }

    fn refresh_accounts(&mut self) {
        if !prepare_scan(&mut self.account_scan, &mut self.retired_account_scans) {
            return;
        }
        let steam_root = self.steam_root.clone();
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let steam = SteamService;
            let roots: Vec<String> = match steam_root {
                Some(root) if Path::new(&root).is_dir() => {
                    vec![fsutil::absolute_lossy(Path::new(&root))
                        .to_string_lossy()
                        .to_string()]
                }
                Some(_) => Vec::new(),
                None => steam.get_roots(None),
            };
            let (accounts, warnings) = steam.get_accounts_with_warnings(&roots);
            let _ = sender.send((roots.len(), accounts, warnings));
        });
        self.account_scan = Some(PendingScan {
            receiver,
            started: Instant::now(),
            timeout_reported: false,
        });
    }

    fn refresh_displays(&mut self) {
        if !prepare_scan(&mut self.display_scan, &mut self.retired_display_scans) {
            return;
        }
        let provider = Arc::clone(&self.display_provider);
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let _ = sender.send(provider.get_displays());
        });
        self.display_scan = Some(PendingScan {
            receiver,
            started: Instant::now(),
            timeout_reported: false,
        });
    }

    /// Adopts a freshly enumerated display list, preserving the selection.
    fn adopt_displays(&mut self, fresh: Vec<DisplayInfo>) {
        let prior = self
            .displays
            .get(self.selected_display)
            .map(|d| d.device_name.clone());
        self.displays = fresh;
        // Labels are static per enumeration; format them once here instead
        // of once per display row per frame.
        self.display_labels = self.displays.iter().map(|d| d.label()).collect();
        if self.displays.is_empty() {
            self.selected_display = 0;
        } else {
            let mut index = prior
                .as_ref()
                .and_then(|p| self.displays.iter().position(|d| &d.device_name == p));
            if index.is_none() {
                index = self.displays.iter().position(|d| d.is_primary);
            }
            self.selected_display = index.unwrap_or(0);
        }
        self.display_changed();
    }

    /// Polls the OS for display-topology and High Contrast changes. Runs
    /// only while the window has focus: nothing can be selected in the
    /// background, and focusing the window triggers a fresh poll.
    fn poll_system(&mut self) {
        if self.last_display_poll.elapsed() < DISPLAY_POLL_INTERVAL {
            return;
        }
        self.last_display_poll = Instant::now();
        self.refresh_high_contrast();
        self.refresh_displays();
    }

    /// Follows Windows High Contrast while the app runs.
    fn refresh_high_contrast(&mut self) {
        let enabled = self.high_contrast_forced || (self.high_contrast_probe)();
        if enabled == self.palette.high_contrast {
            return;
        }
        self.palette = if enabled {
            ThemePalette::high_contrast()
        } else {
            ThemePalette::for_theme(self.preferences.theme)
        };
        self.theme_applied = false;
    }

    fn poll_scans(&mut self) {
        if let Some(scan) = &self.account_scan {
            match scan.receiver.try_recv() {
                Ok((root_count, accounts, warnings)) => {
                    let elapsed = scan.started.elapsed();
                    self.account_scan = None;
                    self.adopt_accounts(root_count, accounts);
                    self.note_scan("Steam account scan", elapsed);
                    if !warnings.is_empty() {
                        self.status.report_background(Notice::warning("Steam accounts were found, but account names could not be read from one or more Steam installations."));
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.account_scan = None;
                    self.status
                        .report_background(Notice::warning("Steam account scan failed."));
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if let Some(scan) = &self.display_scan {
            match scan.receiver.try_recv() {
                Ok(displays) => {
                    let elapsed = scan.started.elapsed();
                    self.display_scan = None;
                    if displays != self.displays {
                        self.adopt_displays(displays);
                    }
                    self.note_scan("Display scan", elapsed);
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.display_scan = None;
                    self.status
                        .report_background(Notice::warning("Display scan failed."));
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if let Some(scan) = &mut self.account_scan {
            if !scan.timeout_reported && scan.started.elapsed() >= SCAN_TIMEOUT {
                scan.timeout_reported = true;
                self.status.report_background(Notice::warning(
                    "Steam account scan is taking longer than expected.",
                ));
            }
        }
        if let Some(scan) = &mut self.display_scan {
            if !scan.timeout_reported && scan.started.elapsed() >= SCAN_TIMEOUT {
                scan.timeout_reported = true;
                self.status.report_background(Notice::warning(
                    "Display scan is taking longer than expected.",
                ));
            }
        }
        if self
            .scan_notice
            .as_ref()
            .is_some_and(|(_, time)| time.elapsed() >= SCAN_NOTICE_DURATION)
        {
            self.scan_notice = None;
        }
    }

    fn note_scan(&mut self, operation: &str, elapsed: Duration) {
        if elapsed >= SLOW_SCAN_THRESHOLD {
            self.scan_notice = Some((
                format!("{operation} completed in {:.1} s", elapsed.as_secs_f64()),
                Instant::now(),
            ));
        }
    }

    fn scan_activity(&self) -> Option<&'static str> {
        if self.account_scan.as_ref().is_some_and(|scan| {
            !scan.timeout_reported && scan.started.elapsed() >= SLOW_SCAN_THRESHOLD
        }) {
            Some("Scanning Steam accounts…")
        } else if self.display_scan.as_ref().is_some_and(|scan| {
            !scan.timeout_reported && scan.started.elapsed() >= SLOW_SCAN_THRESHOLD
        }) {
            Some("Scanning displays…")
        } else {
            None
        }
    }

    /// Whether Refresh can start a scan; mirrors F5 and `prepare_scan`,
    /// which replace a scan once it has timed out.
    fn can_refresh_accounts(&self) -> bool {
        if self
            .account_scan
            .as_ref()
            .is_some_and(|scan| !scan.timeout_reported)
        {
            return false;
        }
        // Retired workers that are still blocked keep their slot; finished
        // ones are reclaimed by `prepare_scan` (their results are discarded).
        let blocked = self
            .retired_account_scans
            .iter()
            .filter(|receiver| matches!(receiver.try_recv(), Err(mpsc::TryRecvError::Empty)))
            .count();
        blocked + usize::from(self.account_scan.is_some()) < 2
    }
    fn scans_need_fast_poll(&self) -> bool {
        self.account_scan
            .as_ref()
            .is_some_and(|scan| !scan.timeout_reported)
            || self
                .display_scan
                .as_ref()
                .is_some_and(|scan| !scan.timeout_reported)
    }

    fn open_diagnostics(&mut self) {
        let version = env!("CARGO_PKG_VERSION");
        let report = DiagnosticService::create_with_options(
            &self.configs,
            version,
            &self.displays,
            self.discovered_root_count,
            self.accounts.len(),
            self.editor.path(),
            self.palette.high_contrast,
        );
        self.diagnostics_summary = DiagnosticService::to_summary(&report);
        self.diagnostics_json = DiagnosticService::to_json(&report);
        self.show_diagnostics = true;
    }

    /// Schedules writing the in-memory preferences. Saving also validates
    /// every recent file, so it stays off the path of each selection.
    fn save_preferences(&mut self) {
        self.preferences_save_due = Some(Instant::now() + PREFERENCES_SAVE_DELAY);
    }

    /// Writes preferences once their quiet period has passed, and otherwise
    /// wakes the UI when it will have.
    fn save_due_preferences(&mut self, ctx: &egui::Context) {
        let Some(due) = self.preferences_save_due else {
            return;
        };
        let now = Instant::now();
        if now >= due {
            self.flush_preferences();
        } else {
            ctx.request_repaint_after(due - now);
        }
    }

    /// Writes pending preference changes now and records whether that worked.
    fn flush_preferences(&mut self) {
        if self.preferences_save_due.take().is_none() {
            return;
        }
        let saved = self.preferences_service().save_selection_with_theme(
            self.preferences.last_account_id.as_deref(),
            self.preferences.last_config_path.as_deref().map(Path::new),
            &self.preferences.recent_config_paths,
            self.preferences.theme,
            self.settings_path_opt(),
        );
        self.status
            .preferences_saved(saved.warning.clone().map_or(Ok(()), Err));
        self.preferences = Preferences {
            warning: None,
            ..saved
        };
    }

    /// Draws the settings and preview cards side by side at equal height in
    /// a single pass. Each card reports its natural height; when the tallest
    /// one changes, the pass is discarded and egui redraws the frame with the
    /// new shared height. Input is consumed only by the first pass, so a
    /// click is never processed twice.
    fn show_main_cards(&mut self, ui: &mut egui::Ui) -> [(egui::Rect, egui::Rect); 2] {
        let bounds = ui.available_rect_before_wrap();
        let (left_rect, right_rect) = fixed_column_rects(bounds, ui.spacing().item_spacing.x, 16.0);
        let shared_height = self.card_height;
        let column = |rect| {
            egui::UiBuilder::new()
                .max_rect(rect)
                .layout(egui::Layout::top_down(egui::Align::Min))
        };
        let (left_actual, left_natural) = ui
            .scope_builder(column(left_rect), |ui| {
                ui.set_clip_rect(left_rect);
                self.settings_card(ui, shared_height)
            })
            .inner;
        let (right_actual, right_natural) = ui
            .scope_builder(column(right_rect), |ui| {
                ui.set_clip_rect(right_rect);
                self.summary_card(ui, shared_height)
            })
            .inner;
        let natural = left_natural.max(right_natural);
        if (natural - shared_height).abs() > 0.5 {
            self.card_height = natural;
            ui.ctx().request_discard("card heights changed");
            // Style changes can need more passes than `CARD_PASSES` allows;
            // the next frame then finishes equalizing.
            ui.ctx().request_repaint();
        }
        [(left_rect, left_actual), (right_rect, right_actual)]
    }
}

impl eframe::App for ResEditApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.initialized {
            ctx.options_mut(|options| options.max_passes = CARD_PASSES);
            self.initialize_data();
        }
        if !self.theme_applied {
            self.palette.apply(ctx, ctx.screen_rect().width() < 900.0);
            if let Some(icon) = self.palette.window_icon() {
                #[cfg(windows)]
                if let Err(error) = self.taskbar_icon.update(_frame, &icon) {
                    self.status.report(Notice::warning(format!(
                        "Taskbar icon could not be updated: {error}"
                    )));
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Icon(Some(icon)));
            }
            self.theme_applied = true;
        }
        self.poll_scans();
        self.save_due_preferences(ctx);
        let focused = ctx.input(|i| i.focused);
        if focused {
            self.poll_system();
        }
        // Background scans need quick polling. Otherwise wake only while
        // focused; regaining focus repaints and triggers the next poll.
        if self.scans_need_fast_poll() {
            ctx.request_repaint_after(Duration::from_millis(50));
        } else if focused {
            ctx.request_repaint_after(DISPLAY_POLL_INTERVAL);
        }
        let modal_open = self.show_backups || self.show_diagnostics || self.confirmation.is_some();
        if !modal_open && ctx.input(|i| i.key_pressed(egui::Key::F5)) {
            self.refresh_accounts();
        }
        // Esc closes the topmost modal; diagnostics sits above backups. An
        // open confirmation handles Esc itself (see `confirmation_prompt`).
        if self.confirmation.is_none() && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            if self.show_diagnostics {
                self.show_diagnostics = false;
            } else if self.show_backups {
                self.show_backups = false;
                self.confirm_restore = false;
            }
        }
        // Exactly Ctrl+S: Ctrl+Shift+S and Ctrl+Alt+S do not apply.
        if !modal_open
            && ctx.input(|i| {
                i.modifiers.matches_exact(egui::Modifiers::COMMAND) && i.key_pressed(egui::Key::S)
            })
        {
            self.request_apply();
        }
        self.header_panel(ctx, modal_open);
        self.footer_panel(ctx, modal_open);

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_enabled_ui(!modal_open, |ui| {
                self.show_main_cards(ui);
            });
        });

        self.backups_window(ctx);
        self.diagnostics_window(ctx);
        self.confirmation_prompt(ctx);
    }

    /// Writes preference changes that were still waiting for their quiet
    /// period when the window closed.
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.flush_preferences();
    }
}

fn export_diagnostics(path: &Path, json: &str) -> std::io::Result<()> {
    cs2_resedit_core::fsutil::atomic_write(path, json.as_bytes())
}

fn custom_account(path: &Path) -> SteamAccount {
    SteamAccount {
        // The full path already appears directly below the selector. Keeping it
        // out of the combo label prevents an unbreakable path from widening the
        // left column and pushing the Preview card off-screen.
        display_name: "Custom file (manually selected)".to_string(),
        account_id: String::new(),
        steam_id64: 0,
        persona_name: "Custom file".to_string(),
        account_name: None,
        most_recent: false,
        config_path: path.to_string_lossy().to_string(),
        has_config: true,
        last_write_time: fsutil::file_mtime_nanos(path),
    }
}

fn fixed_column_rects(bounds: egui::Rect, gap: f32, right_gutter: f32) -> (egui::Rect, egui::Rect) {
    let right_edge = (bounds.max.x - right_gutter).max(bounds.min.x);
    let usable = (right_edge - bounds.min.x - gap).max(0.0);
    let left_width = usable * 0.60;
    let left = egui::Rect::from_min_max(
        bounds.min,
        egui::pos2(bounds.min.x + left_width, bounds.max.y),
    );
    let right = egui::Rect::from_min_max(
        egui::pos2(left.max.x + gap, bounds.min.y),
        egui::pos2(right_edge, bounds.max.y),
    );
    (left, right)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cs2_resedit_core::VideoConfigState;

    #[test]
    fn account_refresh_surfaces_new_load_errors_and_preference_warnings() {
        for preference_failure in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let config = dir.path().join("cs2_video.txt");
            let settings = dir.path().join("settings.json");
            std::fs::write(&config, "\"setting.defaultres\" \"1920\"\n\"setting.defaultresheight\" \"1080\"\n\"setting.aspectratiomode\" \"1\"\n").unwrap();
            let mut app = ResEditApp::new(Some(settings.clone()), None, None);
            assert!(app.load_path(Some(config.clone())));
            app.flush_preferences();
            app.editor.custom_width = "1280".into();
            app.pending_changed();
            app.apply();
            assert!(app.status.text().starts_with("Changes applied."));
            assert!(!app.editor.has_pending_changes());
            if preference_failure {
                std::fs::remove_file(&settings).unwrap();
                std::fs::create_dir(&settings).unwrap();
            } else {
                std::fs::write(&config, "invalid configuration").unwrap();
            }
            app.adopt_accounts(0, vec![custom_account(&config)]);
            app.flush_preferences();
            assert_eq!(app.editor.path(), Some(config.as_path()));
            assert_eq!(app.editor.original().unwrap().width, 1280);
            if preference_failure {
                // The settings warning has its own slot; the apply result
                // for the unchanged file stays visible beside it.
                let warning = app.status.preferences_warning().expect("warning");
                assert!(warning.contains("Preferences could not be saved"));
                assert!(app.status.text().starts_with("Changes applied."));
            } else {
                // A reload error replaces the earlier operation result.
                assert!(
                    app.status.is_warning(),
                    "warning hidden: {}",
                    app.status.text()
                );
                assert!(!app.status.has_operation());
            }
            let shown = app.status.text().to_string();
            app.pending_changed();
            assert_eq!(app.status.text(), shown);
        }
    }

    #[test]
    fn operation_results_survive_edits_displays_and_account_scans() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("cs2_video.txt");
        std::fs::write(&config, "\"setting.defaultres\" \"1920\"\n\"setting.defaultresheight\" \"1080\"\n\"setting.aspectratiomode\" \"1\"\n").unwrap();
        let mut app = ResEditApp::new(Some(dir.path().join("settings.json")), None, None);
        app.load_path(Some(config.clone()));
        app.editor.custom_width = "1280".into();
        app.pending_changed();
        app.apply();
        assert!(app.status.text().starts_with("Changes applied."));
        let applied = app.status.text().to_string();
        app.editor.custom_width = "1600".into();
        app.pending_changed();
        app.display_changed();
        app.adopt_accounts(0, vec![custom_account(&config)]);
        assert_eq!(app.status.text(), applied);
        app.open_backups();
        app.selected_backup = Some(0);
        app.restore_selected_backup();
        assert!(app
            .status
            .text()
            .starts_with("Backup restored successfully."));
        let restored = app.status.text().to_string();
        app.editor.custom_width = "1600".into();
        app.pending_changed();
        app.adopt_displays(Vec::new());
        app.adopt_accounts(0, vec![custom_account(&config)]);
        assert_eq!(app.status.text(), restored);
        std::fs::write(&config, "external edit").unwrap();
        app.apply();
        assert!(app.status.is_warning());
        let failure = app.status.text().to_string();
        let (sender, receiver) = mpsc::channel();
        app.account_scan = Some(PendingScan {
            receiver,
            started: Instant::now(),
            timeout_reported: false,
        });
        sender
            .send((
                0,
                vec![custom_account(&config)],
                vec!["name unavailable".into()],
            ))
            .unwrap();
        app.poll_scans();
        assert_eq!(app.status.text(), failure);
        assert!(app.editor.has_pending_changes());
    }

    #[test]
    fn repeated_timeouts_bound_workers_and_reclaim_completed_slots() {
        let mut app = ResEditApp::new(None, None, None);
        let (first, receiver) = mpsc::channel();
        app.display_scan = Some(PendingScan {
            receiver,
            started: Instant::now() - SCAN_TIMEOUT,
            timeout_reported: true,
        });
        // Preserve the first worker and allow exactly one replacement.
        assert!(prepare_scan(
            &mut app.display_scan,
            &mut app.retired_display_scans
        ));
        let (second, receiver) = mpsc::channel();
        app.display_scan = Some(PendingScan {
            receiver,
            started: Instant::now() - SCAN_TIMEOUT,
            timeout_reported: true,
        });
        for _ in 0..100 {
            app.refresh_displays();
        }
        assert_eq!(app.retired_display_scans.len(), 1);
        first.send(Vec::new()).unwrap();
        assert!(prepare_scan(
            &mut app.display_scan,
            &mut app.retired_display_scans
        ));
        assert_eq!(app.retired_display_scans.len(), 1);
        drop(second);
        assert!(prepare_scan(
            &mut app.display_scan,
            &mut app.retired_display_scans
        ));
        assert!(app.retired_display_scans.is_empty());
    }

    #[test]
    fn fixed_columns_have_hard_bounds_and_a_right_edge_gutter() {
        let bounds = egui::Rect::from_min_size(egui::pos2(8.0, 90.0), egui::vec2(1184.0, 640.0));
        let gap = 10.0;
        let (left, right) = fixed_column_rects(bounds, gap, 16.0);
        assert_eq!(left.min, bounds.min);
        assert_eq!(right.max.x, bounds.max.x - 16.0);
        assert_eq!(right.min.x - left.max.x, gap);
        assert!(left.width() > right.width());
        assert!(right.width() >= 450.0);
        assert!(right.max.x < bounds.max.x);
    }

    #[test]
    fn rendered_cards_stay_inside_columns_with_long_labels() {
        let context = egui::Context::default();
        context.options_mut(|options| options.max_passes = CARD_PASSES);
        let mut app = ResEditApp::new(None, None, None);
        app.accounts.push(custom_account(std::path::Path::new(
            "C:/a/very/long/path/that/should/not/expand/the/configuration/card/cs2_video.txt",
        )));
        app.accounts[0].display_name = "A very long Steam account name ".repeat(12);
        app.selected_account = Some(0);
        app.editor
            .set_path(std::path::PathBuf::from("C:/very/long/".repeat(30)));
        app.editor
            .set_loaded_state(VideoConfigState::new(1280, 960, 0));
        app.editor.reset_pending();
        assert!(matches!(app.editor.edit(), EditState::Unchanged { .. }));

        for theme in UiTheme::ALL {
            app.palette = ThemePalette::for_theme(theme);
            for (width, height, pending_label) in [
                (1200.0, 640.0, "Pending: 1440 × 1080 · 4:3".to_owned()),
                (1024.0, 704.0, "Pending: 1440 × 1080 · 4:3".to_owned()),
                (829.0, 544.0, "Pending: 1440 × 1080 · 4:3".to_owned()),
                (1200.0, 768.0, "Pending: 1440 × 1080 · 4:3 ".repeat(8)),
                (829.0, 544.0, "Pending: 1440 × 1080 · 4:3 ".repeat(8)),
            ] {
                app.palette.apply(&context, width < 900.0);
                app.editor.set_pending_label(pending_label);
                let mut cards = None;
                let _ = context.run(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(width, height),
                        )),
                        ..Default::default()
                    },
                    |ctx| {
                        egui::TopBottomPanel::top("test_header").show(ctx, |ui| {
                            ui.heading("CS2 ResEdit");
                            ui.label("Tune your display settings safely");
                        });
                        egui::TopBottomPanel::bottom("test_footer").show(ctx, |ui| {
                            ui.horizontal(|ui| {
                                ui.label("No pending changes.");
                                let _ = ui.button("Reset");
                                let _ = ui.button("Apply changes");
                            });
                        });
                        egui::CentralPanel::default().show(ctx, |ui| {
                            cards = Some(app.show_main_cards(ui));
                        });
                    },
                );
                let cards = cards.expect("cards rendered");
                assert!(
                    (cards[0].1.max.y - cards[1].1.max.y).abs() <= 1.0,
                    "card bottoms differ: {:?} and {:?}",
                    cards[0].1,
                    cards[1].1
                );
                if width == 1200.0 && height == 640.0 {
                    let unused_height = cards[0].0.max.y - cards[0].1.max.y;
                    assert!(unused_height <= 56.0, "unused height: {unused_height}");
                }
                for (target, actual) in cards {
                    assert!(
                        actual.max.x <= target.max.x + 1.0,
                        "{actual:?} exceeds {target:?}"
                    );
                    assert!(
                        actual.max.y <= target.max.y + 1.0,
                        "{actual:?} exceeds {target:?}"
                    );
                }
            }
        }
    }

    fn viewport(w: f32, h: f32) -> egui::Rect {
        preview_viewport(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::Vec2::new(w, h),
        ))
    }

    #[test]
    fn preview_uses_one_static_scale_across_standard_aspect_families() {
        let bounds = viewport(360.0, 260.0);
        let rect = |w, h| preview_rectangle(bounds, w, h);
        let five_by_four = rect(1280, 1024);
        let four_by_three = rect(1280, 960);
        let sixteen_by_ten = rect(1680, 1050);
        let sixteen_by_nine = rect(1920, 1080);

        for (a, b) in [
            (five_by_four, four_by_three),
            (four_by_three, sixteen_by_ten),
            (sixteen_by_ten, sixteen_by_nine),
        ] {
            assert!((a.height() - b.height()).abs() < 0.001);
        }
        assert!(five_by_four.width() < four_by_three.width());
        assert!(four_by_three.width() < sixteen_by_ten.width());
        assert!(sixteen_by_ten.width() < sixteen_by_nine.width());
        assert_eq!(sixteen_by_nine, bounds);
        assert!((five_by_four.center().x - bounds.width() / 2.0).abs() < 0.001);
        assert!((sixteen_by_nine.center().x - bounds.width() / 2.0).abs() < 0.001);
    }

    #[test]
    fn restore_keeps_preferences_warning_visible() {
        let dir = tempfile::tempdir().expect("tempdir");
        let config = dir.path().join("cs2_video.txt");
        std::fs::write(
            &config,
            "\"setting.defaultres\" \"1920\"\n\"setting.defaultresheight\" \"1080\"\n\"setting.aspectratiomode\" \"1\"\n",
        )
        .expect("write config");
        let mut app = ResEditApp::new(Some(dir.path().to_path_buf()), None, None);
        app.load_path(Some(config.clone()));
        app.flush_preferences();
        assert!(app.status.preferences_warning().is_some());
        app.editor.custom_width = "1280".to_string();
        app.pending_changed();
        app.apply();
        app.open_backups();
        assert!(!app.backups.is_empty());
        app.selected_backup = Some(0);
        app.restore_selected_backup();
        assert!(!app.show_backups);
        assert_eq!(app.status.text(), "Backup restored successfully.");
        assert!(app
            .status
            .preferences_warning()
            .is_some_and(|warning| warning.contains("Preferences could not be saved")));
    }

    #[test]
    fn account_refresh_keeps_unsaved_changes_for_same_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let config = dir.path().join("cs2_video.txt");
        std::fs::write(
            &config,
            "\"setting.defaultres\" \"1920\"\n\"setting.defaultresheight\" \"1080\"\n\"setting.aspectratiomode\" \"1\"\n",
        )
        .unwrap();
        let mut app = ResEditApp::new(None, None, None);
        app.load_path(Some(config.clone()));
        app.editor.custom_width = "1280".to_string();
        app.pending_changed();
        app.adopt_accounts(0, vec![custom_account(&config)]);
        assert_eq!(app.editor.custom_width, "1280");
        assert!(app.editor.has_pending_changes());
        assert!(app.status.text().contains("Unsaved changes kept"));
    }

    #[test]
    fn account_adoption_restores_last_selected_custom_path() {
        let dir = tempfile::tempdir().expect("tempdir");
        let steam = dir.path().join("steam.txt");
        let custom = dir.path().join("custom.txt");
        let text = "\"setting.defaultres\" \"1920\"\n\"setting.defaultresheight\" \"1080\"\n\"setting.aspectratiomode\" \"1\"\n";
        std::fs::write(&steam, text).unwrap();
        std::fs::write(&custom, text).unwrap();
        let mut app = ResEditApp::new(None, None, None);
        app.preferences.last_config_path = Some(custom.to_string_lossy().to_string());
        app.preferences.recent_config_paths = vec![custom.to_string_lossy().to_string()];

        app.adopt_accounts(1, vec![custom_account(&steam)]);

        assert_eq!(app.editor.path(), Some(custom.as_path()));
    }

    #[test]
    fn timed_out_scan_uses_slow_polling_and_can_still_finish() {
        let mut app = ResEditApp::new(None, None, None);
        let (sender, receiver) = mpsc::channel();
        app.display_scan = Some(PendingScan {
            receiver,
            started: Instant::now() - SCAN_TIMEOUT,
            timeout_reported: false,
        });

        app.poll_scans();
        assert!(!app.scans_need_fast_poll());
        assert!(app.status.text().contains("longer than expected"));

        sender.send(Vec::new()).unwrap();
        app.poll_scans();
        assert!(app.display_scan.is_none());
    }

    #[test]
    fn timed_out_scans_are_replaced_and_their_results_dropped() {
        let mut app = ResEditApp::new(None, None, None);
        // Simulate a hung first scan that already timed out.
        let (old_sender, old_receiver) = mpsc::channel::<Vec<DisplayInfo>>();
        app.display_scan = Some(PendingScan {
            receiver: old_receiver,
            started: Instant::now() - SCAN_TIMEOUT,
            timeout_reported: true,
        });
        // The abandoned worker's late result arrives while wedged.
        let stale_display = DisplayInfo {
            device_name: "STALE".to_string(),
            friendly_name: "Stale display".to_string(),
            is_primary: true,
            current_width: 800,
            current_height: 600,
            modes: vec![],
        };
        old_sender.send(vec![stale_display]).unwrap();
        // A refresh must replace the timed-out scan instead of wedging, and
        // the stale queued result dies with the abandoned receiver.
        app.refresh_displays();
        assert_eq!(app.retired_display_scans.len(), 1);
        let replacement = app.display_scan.as_ref().unwrap();
        assert!(!replacement.timeout_reported);
        let started = replacement.started;
        // Fresh (non-timed-out) scans still block replacement.
        app.refresh_displays();
        assert_eq!(app.display_scan.as_ref().unwrap().started, started);
        // The abandoned worker had finished, so its slot is reclaimed and the
        // queued stale result is dropped unread.
        assert!(app.retired_display_scans.is_empty());
        app.poll_scans();
        assert!(app.displays.is_empty());
        assert!(!app.displays.iter().any(|d| d.device_name == "STALE"));
    }

    #[test]
    fn diagnostics_export_reports_io_errors() {
        let dir = tempfile::tempdir().expect("tempdir");
        let output = dir.path().join("diagnostics.json");
        export_diagnostics(&output, "{\"ok\":true}").expect("export");
        assert_eq!(std::fs::read_to_string(output).unwrap(), "{\"ok\":true}");
        assert!(export_diagnostics(dir.path(), "{}").is_err());
    }

    #[test]
    fn display_scan_returns_control_before_provider_finishes() {
        use std::sync::atomic::{AtomicBool, Ordering};

        struct WaitingProvider(Arc<AtomicBool>);
        impl DisplayModeProvider for WaitingProvider {
            fn get_displays(&self) -> Vec<DisplayInfo> {
                while !self.0.load(Ordering::Acquire) {
                    thread::sleep(Duration::from_millis(5));
                }
                Vec::new()
            }
        }

        let release = Arc::new(AtomicBool::new(false));
        let mut app = ResEditApp::new(
            None,
            None,
            Some(Box::new(WaitingProvider(Arc::clone(&release)))),
        );
        app.refresh_displays();
        assert!(app.display_scan.is_some());
        release.store(true, Ordering::Release);
        let deadline = Instant::now() + Duration::from_secs(2);
        while app.display_scan.is_some() && Instant::now() < deadline {
            app.poll_scans();
            thread::sleep(Duration::from_millis(5));
        }
        assert!(app.display_scan.is_none());
    }

    #[test]
    fn startup_preferences_warning_survives_first_load() {
        let dir = tempfile::tempdir().expect("tempdir");
        let settings = dir.path().join("settings.json");
        std::fs::write(&settings, "{ invalid json").unwrap();
        let config = dir.path().join("cs2_video.txt");
        std::fs::write(
            &config,
            "\"setting.defaultres\" \"1920\"\n\"setting.defaultresheight\" \"1080\"\n\"setting.aspectratiomode\" \"1\"\n",
        )
        .unwrap();
        let mut app = ResEditApp::new(Some(settings), None, None);
        app.initialize_data();
        let read_warning = |app: &ResEditApp| {
            app.status
                .preferences_warning()
                .is_some_and(|warning| warning.contains("Preferences could not be loaded"))
        };
        assert!(read_warning(&app));
        // Simulate the auto-select load path from adopt_accounts. Its save
        // succeeds, but the startup read failure stays visible.
        app.selected_account = Some(0);
        app.accounts.push(custom_account(&config));
        app.load_path(Some(config));
        assert_eq!(app.status.text(), "Configuration loaded.");
        assert!(read_warning(&app));
    }

    #[test]
    fn preference_saves_wait_for_a_quiet_period() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, config) = loaded_app(dir.path());
        let settings = dir.path().join("settings.json");
        let ctx = egui::Context::default();
        // Loading schedules a save instead of writing on the spot.
        app.save_due_preferences(&ctx);
        assert!(!settings.exists());
        let first_due = app.preferences_save_due.expect("save scheduled");
        // Further changes push the write back rather than adding writes.
        app.preferences.theme = UiTheme::Paper;
        app.save_preferences();
        assert!(app.preferences_save_due.unwrap() >= first_due);

        app.preferences_save_due = Some(Instant::now() - Duration::from_millis(1));
        app.save_due_preferences(&ctx);
        assert!(app.preferences_save_due.is_none());
        let saved = std::fs::read_to_string(&settings).unwrap();
        assert!(saved.contains("Paper"), "{saved}");
        assert!(saved.contains("cs2_video.txt"), "{saved}");
        assert_eq!(app.editor.path(), Some(config.as_path()));
    }

    #[test]
    fn loading_reports_configuration_loaded_after_the_edit_hint() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, config) = loaded_app(dir.path());
        assert_eq!(app.status.text(), "Configuration loaded.");
        let other = dir.path().join("other.txt");
        std::fs::copy(&config, &other).unwrap();
        app.adopt_accounts(1, vec![custom_account(&other)]);
        assert_eq!(app.editor.path(), Some(config.as_path()), "keeps the file");
        assert_eq!(
            app.status.text(),
            "Steam accounts refreshed. Configuration loaded."
        );

        let mut fresh = ResEditApp::new(Some(dir.path().join("settings.json")), None, None);
        fresh.preferences.last_config_path = None;
        fresh.adopt_accounts(1, vec![custom_account(&other)]);
        assert_eq!(
            fresh.status.text(),
            "Steam accounts refreshed. Configuration loaded."
        );
    }

    #[test]
    fn pending_refresh_preserves_operation_warnings() {
        let mut app = ResEditApp::new(None, None, None);
        app.editor
            .set_loaded_state(VideoConfigState::new(1920, 1080, 1));
        app.editor.aspect_index = 1;
        app.editor.custom_width = "1920".to_string();
        app.editor.custom_height = "1080".to_string();
        app.status
            .report(Notice::warning("Apply failed: disk full"));
        app.pending_changed();
        assert!(app.status.is_warning());
        assert_eq!(app.status.text(), "Apply failed: disk full");
    }

    #[test]
    fn failed_restore_disarms_confirm_dialog() {
        let dir = tempfile::tempdir().expect("tempdir");
        let config = dir.path().join("cs2_video.txt");
        std::fs::write(
            &config,
            "\"setting.defaultres\" \"1920\"\n\"setting.defaultresheight\" \"1080\"\n\"setting.aspectratiomode\" \"1\"\n",
        )
        .unwrap();
        let mut app = ResEditApp::new(None, None, None);
        app.load_path(Some(config.clone()));
        app.editor.custom_width = "1280".to_string();
        app.pending_changed();
        app.apply();
        app.open_backups();
        app.selected_backup = Some(0);
        app.confirm_restore = true;
        // Corrupt the live file so restore fails validation.
        std::fs::write(&config, "invalid").unwrap();
        // Refresh snapshot mismatch path: external edit then restore attempt.
        app.restore_selected_backup();
        assert!(app.status.is_warning());
        assert!(!app.confirm_restore);
    }

    #[test]
    fn invalid_dimensions_keep_footer_operation_status() {
        let mut app = ResEditApp::new(None, None, None);
        app.editor
            .set_loaded_state(VideoConfigState::new(1920, 1080, 1));
        app.editor.aspect_index = 1;
        app.editor.custom_width = "1920".to_string();
        app.editor.custom_height = "1080".to_string();
        app.status
            .report(Notice::warning("Apply failed: disk full"));
        app.editor.custom_width = "not-a-number".to_string();
        app.pending_changed();
        assert!(matches!(app.editor.edit(), EditState::Invalid));
        assert!(!app.editor.validation().is_empty());
        assert!(app.status.is_warning());
        assert_eq!(app.status.text(), "Apply failed: disk full");
        assert!(!app.editor.can_apply());
        assert_eq!(
            app.editor.apply_blocker_reason(),
            Some("Enter valid dimensions")
        );
    }

    /// Guards UI strings against tofu: every non-ASCII glyph in the UI sources
    /// must exist in egui's bundled proportional fallback chain
    /// (Ubuntu-Light, NotoEmoji-Regular, emoji-icon-font), as verified with
    /// fontTools against epaint_default_fonts 0.32.3. Notably U+2713 is
    /// missing from all bundled fonts and must never be used.
    #[test]
    fn ui_glyphs_are_covered_by_bundled_fonts() {
        const COVERED: &[char] = &['·', '—', '–', '×', '✔', '✖', 'ℹ', '»', '…', '○', '•', '⚠'];
        let missing: Vec<char> = include_str!("app.rs")
            .chars()
            .chain(include_str!("app/panels.rs").chars())
            .chain(include_str!("theme.rs").chars())
            .chain(include_str!("main.rs").chars())
            .chain(include_str!("../../core/src/models.rs").chars())
            .filter(|c| !c.is_ascii())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .filter(|c| !COVERED.contains(c))
            .collect();
        assert!(missing.is_empty(), "uncovered UI glyphs: {missing:?}");
    }

    #[test]
    fn footer_actions_stay_visible_with_long_messages() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _) = loaded_app(dir.path());
        app.editor.custom_width = "1280".into();
        app.pending_changed();
        app.status.report(Notice::warning(
            "The configuration changed outside CS2 ResEdit. Reload it before applying changes. "
                .repeat(3),
        ));
        app.status.preferences_saved(Err(
            "Preferences could not be saved: access to the settings folder was denied. ".repeat(2),
        ));
        // Clicking Apply while CS2 "runs" only opens the prompt, so the test
        // can prove the click landed without writing the file.
        app.game_probe = || Some(true);

        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let (width, height) = (829.0, 544.0);
        let render = |app: &mut ResEditApp, events: Vec<egui::Event>| {
            ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(width, height),
                    )),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    app.footer_panel(ctx, false);
                    egui::CentralPanel::default().show(ctx, |ui| {
                        app.show_main_cards(ui);
                    });
                },
            )
        };
        let mut buttons = std::collections::HashMap::new();
        for _ in 0..3 {
            let output = render(&mut app, vec![]);
            for (_, node) in output.platform_output.accesskit_update.unwrap().nodes {
                if let (Some(label), Some(bounds)) = (node.label(), node.bounds()) {
                    if label == "Apply changes" || label == "Reset" {
                        buttons.insert(label.to_string(), bounds);
                    }
                }
            }
        }
        for name in ["Apply changes", "Reset"] {
            let rect = buttons
                .get(name)
                .unwrap_or_else(|| panic!("{name} missing"));
            assert!(
                rect.x0 >= 0.0 && rect.x1 <= f64::from(width) && rect.y1 <= f64::from(height),
                "{name} outside the window: {rect:?}"
            );
            assert!(rect.x1 - rect.x0 > 20.0, "{name} squeezed: {rect:?}");
        }
        let apply = buttons["Apply changes"];
        let pos = egui::pos2(
            ((apply.x0 + apply.x1) / 2.0) as f32,
            ((apply.y0 + apply.y1) / 2.0) as f32,
        );
        for pressed in [true, false] {
            render(
                &mut app,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
        }
        assert_eq!(
            app.confirmation,
            Some(Confirmation::GameRunning(GuardedWrite::Apply))
        );
    }

    fn loaded_app(dir: &Path) -> (ResEditApp, PathBuf) {
        let config = dir.join("cs2_video.txt");
        std::fs::write(
            &config,
            "\"setting.defaultres\" \"1920\"\n\"setting.defaultresheight\" \"1080\"\n\"setting.aspectratiomode\" \"1\"\n",
        )
        .unwrap();
        let mut app = ResEditApp::new(Some(dir.join("settings.json")), None, None);
        app.game_probe = || Some(false);
        assert!(app.load_path(Some(config.clone())));
        (app, config)
    }

    #[test]
    fn running_game_asks_before_apply_and_restore() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, config) = loaded_app(dir.path());
        let original = std::fs::read(&config).unwrap();
        app.editor.custom_width = "1280".into();
        app.pending_changed();

        app.game_probe = || Some(true);
        app.request_apply();
        assert_eq!(
            app.confirmation,
            Some(Confirmation::GameRunning(GuardedWrite::Apply))
        );
        assert_eq!(std::fs::read(&config).unwrap(), original);
        app.cancel_confirmation();
        assert!(app.editor.can_apply(), "cancel keeps the pending edit");

        app.request_apply();
        app.confirm();
        assert!(app.status.text().starts_with("Changes applied."));
        assert!(app.confirmation.is_none());

        app.open_backups();
        app.selected_backup = Some(0);
        app.request_restore();
        assert_eq!(
            app.confirmation,
            Some(Confirmation::GameRunning(GuardedWrite::Restore))
        );
        app.confirm();
        assert!(app
            .status
            .text()
            .starts_with("Backup restored successfully."));

        // An unreadable process list does not block the write.
        app.game_probe = || None;
        app.editor.custom_width = "1600".into();
        app.pending_changed();
        app.request_apply();
        assert!(app.confirmation.is_none());
        assert!(app.status.text().starts_with("Changes applied."));
    }

    #[test]
    fn discarding_pending_edits_requires_confirmation() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, config) = loaded_app(dir.path());
        app.accounts.push(custom_account(&config));
        let other = dir.path().join("other").join("cs2_video.txt");
        std::fs::create_dir_all(other.parent().unwrap()).unwrap();
        std::fs::copy(&config, &other).unwrap();
        app.accounts.push(custom_account(&other));
        app.selected_account = Some(0);

        // Without pending edits, actions run immediately.
        app.request_discard(DiscardAction::SelectAccount(1));
        assert!(app.confirmation.is_none());
        assert_eq!(app.editor.path(), Some(other.as_path()));

        app.editor.custom_width = "1280".into();
        app.pending_changed();
        // Re-selecting the active entry neither prompts nor reloads.
        app.request_discard(DiscardAction::SelectAccount(1));
        assert!(app.confirmation.is_none());
        assert!(app.editor.has_pending_changes());

        let config_entry = config.to_string_lossy().to_string();
        for (action, queued) in [
            (
                DiscardAction::SelectAccount(0),
                DiscardAction::SelectConfig(config_entry),
            ),
            (DiscardAction::Reload, DiscardAction::Reload),
            (
                DiscardAction::OpenFile(config.clone()),
                DiscardAction::OpenFile(config.clone()),
            ),
        ] {
            app.request_discard(action);
            assert_eq!(app.confirmation, Some(Confirmation::DiscardEdits(queued)));
            app.cancel_confirmation();
            assert!(app.editor.has_pending_changes(), "cancel keeps the edit");
            assert_eq!(app.editor.path(), Some(other.as_path()));
        }

        // A scan that reorders the list while the prompt is open must not
        // redirect the switch to whichever account now has that index.
        app.request_discard(DiscardAction::SelectAccount(0));
        app.accounts.reverse();
        app.selected_account = Some(0);
        app.confirm();
        assert_eq!(app.editor.path(), Some(config.as_path()));
        assert_eq!(app.selected_account, Some(1));
        assert!(!app.editor.has_pending_changes());

        // A queued account that disappeared is reported, and edits survive.
        app.editor.custom_width = "1280".into();
        app.pending_changed();
        app.request_discard(DiscardAction::SelectAccount(0));
        app.accounts.remove(0);
        app.selected_account = Some(0);
        app.confirm();
        assert_eq!(app.editor.path(), Some(config.as_path()));
        assert!(app.editor.has_pending_changes());
        assert!(
            app.status.text().contains("no longer listed"),
            "{}",
            app.status.text()
        );
    }

    #[test]
    fn refresh_stays_disabled_while_both_worker_slots_are_blocked() {
        let mut app = ResEditApp::new(None, None, None);
        let (blocked_sender, blocked) = mpsc::channel();
        app.retired_account_scans.push(blocked);
        let (_sender, receiver) = mpsc::channel();
        app.account_scan = Some(PendingScan {
            receiver,
            started: Instant::now(),
            timeout_reported: true,
        });
        assert!(!app.can_refresh_accounts());
        // Once the retired worker finishes, its slot is free again.
        drop(blocked_sender);
        assert!(app.can_refresh_accounts());
    }

    #[test]
    fn palette_follows_windows_high_contrast_changes() {
        let mut app = ResEditApp::new(None, None, None);
        app.high_contrast_forced = false;
        app.preferences.theme = UiTheme::Canopy;
        app.high_contrast_probe = || true;
        app.refresh_high_contrast();
        assert!(app.palette.high_contrast);
        assert!(!app.theme_applied);
        app.theme_applied = true;
        app.refresh_high_contrast();
        assert!(app.theme_applied, "unchanged state does not restyle");
        app.high_contrast_probe = || false;
        app.refresh_high_contrast();
        assert_eq!(app.palette, ThemePalette::for_theme(UiTheme::Canopy));
    }

    #[test]
    fn refresh_is_available_again_after_a_scan_times_out() {
        let mut app = ResEditApp::new(None, None, None);
        assert!(app.can_refresh_accounts());
        let (_sender, receiver) = mpsc::channel();
        app.account_scan = Some(PendingScan {
            receiver,
            started: Instant::now(),
            timeout_reported: false,
        });
        assert!(!app.can_refresh_accounts());
        app.account_scan.as_mut().unwrap().timeout_reported = true;
        assert!(app.can_refresh_accounts());
    }
}
