//! Desktop UI and application state for CS2 ResEdit.
//!
//! The layout has a header, a settings card and a
//! preview card (side by side in the fixed-size window), and a footer
//! with status text plus Reset / Apply actions.

use cs2_resedit_core::{
    fsutil, CoreError, DiagnosticService, DisplayInfo, DisplayModeProvider, DisplayProvider,
    Preferences, PreferencesService, Resolution, ResolutionCatalog, SteamAccount, SteamService,
    UiTheme, UpdateResult, VideoConfigService, VideoConfigSnapshot, VideoConfigState,
};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::{Duration, Instant};

use crate::theme::ThemePalette;

mod controller;
mod panels;
#[cfg(test)]
use panels::{preview_rectangle, preview_viewport};

pub(crate) use controller::EditState;

const ASPECT_LABELS: [&str; 3] = ["4:3 / 5:4", "16:9", "16:10"];
const DISPLAY_POLL_INTERVAL: Duration = Duration::from_secs(2);
const SLOW_SCAN_THRESHOLD: Duration = Duration::from_millis(150);
const SCAN_NOTICE_DURATION: Duration = Duration::from_secs(4);
const SCAN_TIMEOUT: Duration = Duration::from_secs(10);

/// Background enumeration request. `generation` matches the coordinator
/// sequence at spawn time; results from an older generation are stale and
/// must be dropped instead of adopted (see `poll_scans`).
struct PendingScan<T> {
    receiver: mpsc::Receiver<T>,
    started: Instant,
    timeout_reported: bool,
    generation: u64,
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
    aspect_index: usize,
    preset_choices: Vec<PresetChoice>,
    selected_preset: usize,
    custom_width: String,
    custom_height: String,
    supported_modes: HashSet<(i32, i32)>,

    selected_path: Option<PathBuf>,
    original: Option<cs2_resedit_core::VideoConfigState>,
    snapshot: Option<VideoConfigSnapshot>,
    // Cached derived state, refreshed by pending_changed(); the frame loop
    // never re-parses or re-formats these.
    edit: EditState,
    /// Last valid pending resolution, kept so the aspect preview stays stable
    /// while custom-dimension typing passes through intermediate invalid states.
    /// Never used for Apply; `edit` alone gates `can_apply()`.
    preview_fallback: Option<Resolution>,
    current_label: String,
    pending_label: String,
    create_backup: bool,
    status: String,
    status_warning: bool,
    external_change_notice: Option<String>,
    validation: String,
    availability: String,
    availability_ok: Option<bool>,

    show_backups: bool,
    backups: Vec<cs2_resedit_core::BackupInfo>,
    selected_backup: Option<usize>,
    confirm_restore: bool,
    show_diagnostics: bool,
    diagnostics_summary: String,
    diagnostics_json: String,
    discovered_root_count: usize,
    /// Preferences-load warning from startup. `load_path` saves preferences
    /// on every successful load, and a save result carries `warning: None`
    /// unless the save itself fails — without this field the original
    /// corrupt-settings warning would be silently discarded after the first
    /// save (see `load_path`).
    load_warning: Option<String>,
    display_labels: Vec<String>,
    initialized: bool,
    theme_applied: bool,
    #[cfg(windows)]
    taskbar_icon: crate::taskbar_icon::TaskbarIcon,
    last_display_poll: Instant,
    account_scan: Option<PendingScan<AccountScanResult>>,
    display_scan: Option<PendingScan<Vec<DisplayInfo>>>,
    scan_notice: Option<(String, Instant)>,
    /// Generation-tagged scan coordinator sequences. A refresh that finds a
    /// timed-out scan replaces it with a newer generation; late results from
    /// the abandoned generation are dropped on arrival so a hung worker can
    /// never wedge future scans or overwrite newer state.
    account_scan_seq: u64,
    display_scan_seq: u64,
}

#[derive(Debug, Clone)]
struct PresetChoice {
    resolution: Option<Resolution>,
    label: String,
}

impl ResEditApp {
    pub fn new(
        settings_path: Option<PathBuf>,
        steam_root: Option<String>,
        display_provider: Option<Box<dyn DisplayModeProvider>>,
    ) -> Self {
        let high_contrast =
            std::env::var("CS2_RESEDIT_HIGH_CONTRAST").is_ok_and(|v| !v.is_empty() && v != "0");
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
            aspect_index: 0,
            preset_choices: vec![PresetChoice {
                resolution: None,
                label: "Custom resolution".to_string(),
            }],
            selected_preset: 0,
            custom_width: "1280".to_string(),
            custom_height: "960".to_string(),
            supported_modes: HashSet::new(),
            selected_path: None,
            original: None,
            snapshot: None,
            edit: EditState::NoFile,
            preview_fallback: None,
            current_label: "—".to_string(),
            pending_label: "—".to_string(),
            create_backup: true,
            status: String::new(),
            status_warning: false,
            external_change_notice: None,
            validation: String::new(),
            availability: String::new(),
            availability_ok: None,
            show_backups: false,
            backups: Vec::new(),
            selected_backup: None,
            confirm_restore: false,
            show_diagnostics: false,
            diagnostics_summary: String::new(),
            diagnostics_json: String::new(),
            discovered_root_count: 0,
            load_warning: None,
            display_labels: Vec::new(),
            initialized: false,
            theme_applied: false,
            #[cfg(windows)]
            taskbar_icon: crate::taskbar_icon::TaskbarIcon::default(),
            last_display_poll: Instant::now(),
            account_scan: None,
            display_scan: None,
            scan_notice: None,
            account_scan_seq: 0,
            display_scan_seq: 0,
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
        // Keep the read-time warning aside: `load_path` saves preferences on
        // every successful load and would otherwise clobber it with a fresh
        // `warning: None` result.
        self.load_warning = self.preferences.warning.clone();
        if let Some(warning) = self.load_warning.clone() {
            self.set_status(&warning, true);
        }
        self.refresh_displays();
        self.aspect_index = 0;
        let initial = ResolutionCatalog::recommended_preset(0).expect("mode 0 preset");
        self.populate_preset_choices(0, Some(initial.width), Some(initial.height));
        self.custom_width = initial.width.to_string();
        self.custom_height = initial.height.to_string();
        self.refresh_accounts();
        self.initialized = true;
    }

    fn refresh_accounts(&mut self) {
        // A scan that already timed out is abandoned: bumping the generation
        // orphans its late result (dropped in `poll_scans`) and lets a fresh
        // worker proceed instead of wedging refreshes forever.
        if let Some(scan) = &self.account_scan {
            if !scan.timeout_reported {
                return;
            }
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
        self.account_scan_seq += 1;
        self.account_scan = Some(PendingScan {
            receiver,
            started: Instant::now(),
            timeout_reported: false,
            generation: self.account_scan_seq,
        });
    }

    fn refresh_displays(&mut self) {
        if let Some(scan) = &self.display_scan {
            if !scan.timeout_reported {
                return;
            }
        }
        let provider = Arc::clone(&self.display_provider);
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let _ = sender.send(provider.get_displays());
        });
        self.display_scan_seq += 1;
        self.display_scan = Some(PendingScan {
            receiver,
            started: Instant::now(),
            timeout_reported: false,
            generation: self.display_scan_seq,
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

    /// Polls the OS for topology changes, mirroring the C# app's
    /// WM_DISPLAYCHANGE handling; adopts the new list only on change.
    fn poll_displays(&mut self) {
        if self.last_display_poll.elapsed() < DISPLAY_POLL_INTERVAL {
            return;
        }
        self.last_display_poll = Instant::now();
        self.refresh_displays();
    }

    fn poll_scans(&mut self) {
        if let Some(scan) = &self.account_scan {
            match scan.receiver.try_recv() {
                Ok((root_count, accounts, warnings)) => {
                    let elapsed = scan.started.elapsed();
                    let current = scan.generation == self.account_scan_seq;
                    self.account_scan = None;
                    // Stale generations (abandoned after a timeout) must not
                    // overwrite state adopted by a newer scan.
                    if current {
                        self.adopt_accounts(root_count, accounts);
                        self.note_scan("Steam account scan", elapsed);
                        if !warnings.is_empty() {
                            self.set_status("Steam accounts were found, but account names could not be read from one or more Steam installations.", true);
                        }
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    let current = scan.generation == self.account_scan_seq;
                    self.account_scan = None;
                    if current {
                        self.set_status("Steam account scan failed.", true);
                    }
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if let Some(scan) = &self.display_scan {
            match scan.receiver.try_recv() {
                Ok(displays) => {
                    let elapsed = scan.started.elapsed();
                    let current = scan.generation == self.display_scan_seq;
                    self.display_scan = None;
                    if current {
                        if displays != self.displays {
                            self.adopt_displays(displays);
                        }
                        self.note_scan("Display scan", elapsed);
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    let current = scan.generation == self.display_scan_seq;
                    self.display_scan = None;
                    if current {
                        self.set_status("Display scan failed.", true);
                    }
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if let Some(scan) = &mut self.account_scan {
            if !scan.timeout_reported && scan.started.elapsed() >= SCAN_TIMEOUT {
                scan.timeout_reported = true;
                self.set_status("Steam account scan is taking longer than expected.", true);
            }
        }
        if let Some(scan) = &mut self.display_scan {
            if !scan.timeout_reported && scan.started.elapsed() >= SCAN_TIMEOUT {
                scan.timeout_reported = true;
                self.set_status("Display scan is taking longer than expected.", true);
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
            self.selected_path.as_deref(),
            self.palette.high_contrast,
        );
        self.diagnostics_summary = DiagnosticService::to_summary(&report);
        self.diagnostics_json = DiagnosticService::to_json(&report);
        self.show_diagnostics = true;
    }

    fn set_status(&mut self, message: &str, warning: bool) {
        self.status = message.to_string();
        self.status_warning = warning;
    }

    fn show_main_cards(&mut self, ui: &mut egui::Ui) -> [(egui::Rect, egui::Rect); 2] {
        let bounds = ui.available_rect_before_wrap();
        let (left_rect, right_rect) = fixed_column_rects(bounds, ui.spacing().item_spacing.x, 16.0);
        // Measurement must not share interactive IDs or process clicks. Otherwise
        // a dropdown click toggles its popup in both passes and immediately closes it.
        let measured_left = ui
            .scope_builder(
                egui::UiBuilder::new()
                    .id_salt("measure_settings")
                    .invisible()
                    .max_rect(left_rect)
                    .sizing_pass()
                    .layout(egui::Layout::top_down(egui::Align::Min)),
                |ui| self.settings_card(ui, 0.0),
            )
            .inner;
        let measured_right = ui
            .scope_builder(
                egui::UiBuilder::new()
                    .id_salt("measure_preview")
                    .invisible()
                    .max_rect(right_rect)
                    .sizing_pass()
                    .layout(egui::Layout::top_down(egui::Align::Min)),
                |ui| self.summary_card(ui, 0.0, None),
            )
            .inner;
        let shared_height = measured_left.height().max(measured_right.0.height());
        let left_actual = ui
            .scope_builder(
                egui::UiBuilder::new()
                    .max_rect(left_rect)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
                |ui| {
                    ui.set_clip_rect(left_rect);
                    self.settings_card(ui, shared_height)
                },
            )
            .inner;
        let right_actual = ui
            .scope_builder(
                egui::UiBuilder::new()
                    .max_rect(right_rect)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
                |ui| {
                    ui.set_clip_rect(right_rect);
                    self.summary_card(ui, shared_height, Some(measured_right.1))
                        .0
                },
            )
            .inner;
        [(left_rect, left_actual), (right_rect, right_actual)]
    }
}

impl eframe::App for ResEditApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.initialized {
            self.initialize_data();
        }
        if !self.theme_applied {
            self.palette.apply(ctx, ctx.screen_rect().width() < 900.0);
            if let Some(icon) = self.palette.window_icon() {
                #[cfg(windows)]
                if let Err(error) = self.taskbar_icon.update(_frame, &icon) {
                    self.set_status(&format!("Taskbar icon could not be updated: {error}"), true);
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Icon(Some(icon)));
            }
            self.theme_applied = true;
        }
        self.poll_scans();
        self.poll_displays();
        ctx.request_repaint_after(if self.scans_need_fast_poll() {
            Duration::from_millis(50)
        } else {
            DISPLAY_POLL_INTERVAL
        });
        let modal_open = self.show_backups || self.show_diagnostics;
        if !modal_open && ctx.input(|i| i.key_pressed(egui::Key::F5)) {
            self.refresh_accounts();
        }
        // Esc closes the topmost modal; diagnostics sits above backups.
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            if self.show_diagnostics {
                self.show_diagnostics = false;
            } else if self.show_backups {
                self.show_backups = false;
                self.confirm_restore = false;
            }
        }
        if !modal_open
            && ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::S))
            && self.can_apply()
        {
            self.apply();
        }
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
                            self.apply();
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

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_enabled_ui(!modal_open, |ui| {
                self.show_main_cards(ui);
            });
        });

        if self.show_backups {
            let mut open = self.show_backups;
            egui::Window::new("Configuration backups")
                .open(&mut open)
                .resizable(true)
                .default_size([560.0, 420.0])
                .min_size([420.0, 300.0])
                .show(ctx, |ui| {
                                if let Some(comparison) = &self.external_change_notice {
                                    ui.colored_label(palette.warning, comparison);
                                    ui.separator();
                                }
                    if self.backups.is_empty() {
                        ui.label("No editor backups were found.");
                        ui.label(
                            "Apply a change with backups enabled to create one. \
                             Backups are stored next to the configuration.",
                        );
                    } else {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            for (index, backup) in self.backups.iter().enumerate() {
                                let name = ResolutionCatalog::mode_name(
                                    backup.state.aspect_mode,
                                    backup.state.width,
                                    backup.state.height,
                                )
                                .unwrap_or("?");
                                let created = chrono::DateTime::from_timestamp_nanos(backup.created)
                                    .with_timezone(&chrono::Local);
                                let selected = self.selected_backup == Some(index);
                                if ui
                                    .selectable_label(
                                        selected,
                                        format!(
                                            "{}  ·  {} × {}  ·  {name}",
                                            created.format("%Y-%m-%d %H:%M:%S"),
                                            backup.state.width,
                                            backup.state.height
                                        ),
                                    )
                                    .clicked()
                                {
                                    self.selected_backup = Some(index);
                                    self.confirm_restore = false;
                                }
                            }
                        });
                        if let Some(index) = self.selected_backup {
                            if let Some(backup) = self.backups.get(index) {
                                let mode = ResolutionCatalog::mode_name(
                                    backup.state.aspect_mode,
                                    backup.state.width,
                                    backup.state.height,
                                )
                                .unwrap_or("?");
                                let created = chrono::DateTime::from_timestamp_nanos(backup.created)
                                    .with_timezone(&chrono::Local);
                                ui.label(format!(
                                    "{}  ·  {} × {}  ·  {mode}",
                                    created.format("%Y-%m-%d %H:%M:%S"),
                                    backup.state.width,
                                    backup.state.height
                                ));
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(backup.path.clone()).monospace().small(),
                                    )
                                    .truncate(),
                                )
                                .on_hover_text(backup.path.clone());
                                if let Some(original) = self.original {
                                    if original != backup.state {
                                        ui.label(format!(
                                            "Current file: {} × {} · {} — restoring will overwrite these values.",
                                            original.width,
                                            original.height,
                                            ResolutionCatalog::mode_name(
                                                original.aspect_mode,
                                                original.width,
                                                original.height
                                            )
                                            .unwrap_or("?"),
                                        ));
                                    } else {
                                        ui.label("Backup matches the current file.");
                                    }
                                }
                            }
                        }
                        if self.confirm_restore {
                            ui.label("Restore this backup? A rollback backup of the active file will be created.");
                            ui.horizontal(|ui| {
                                if ui.button("Confirm restore").clicked() {
                                    self.restore_selected_backup();
                                }
                                if ui.button("Cancel").clicked() {
                                    self.confirm_restore = false;
                                }
                            });
                        } else {
                            if ui
                                .add_enabled(
                                    self.selected_backup.is_some(),
                                    egui::Button::new("Restore selected"),
                                )
                                .clicked()
                            {
                                self.confirm_restore = true;
                            }
                        }
                    }
                });
            self.show_backups &= open;
            if !open {
                self.confirm_restore = false;
            }
        }

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

fn describe_external_change(
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

fn describe_video_state(state: VideoConfigState) -> String {
    let aspect = ResolutionCatalog::mode_name(state.aspect_mode, state.width, state.height)
        .unwrap_or("unknown aspect");
    format!("{} × {} ({aspect})", state.width, state.height)
}
#[cfg(test)]
mod tests {
    use super::*;

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
        let mut app = ResEditApp::new(None, None, None);
        app.accounts.push(custom_account(std::path::Path::new(
            "C:/a/very/long/path/that/should/not/expand/the/configuration/card/cs2_video.txt",
        )));
        app.accounts[0].display_name = "A very long Steam account name ".repeat(12);
        app.selected_account = Some(0);
        app.selected_path = Some(std::path::PathBuf::from("C:/very/long/".repeat(30)));
        app.current_label = "Current: 1440 × 1080 · 4:3".to_owned();
        app.availability = "Reported for selected display".to_owned();
        app.original = Some(VideoConfigState {
            width: 1280,
            height: 960,
            aspect_mode: 0,
        });
        app.edit = EditState::Unchanged {
            resolution: Resolution::new(1280, 960, "4:3", 0),
        };
        app.populate_preset_choices(0, Some(1280), Some(960));

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
                app.pending_label = pending_label;
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
    fn aspect_selection_filters_preset_catalog() {
        let mut app = ResEditApp::new(None, None, None);
        app.supported_modes.clear();
        for (mode, expected_first, expected_ratio) in [
            (0, "1280x960", "(4:3)"),
            (1, "1920x1080", "(16:9)"),
            (2, "1680x1050", "(16:10)"),
        ] {
            app.on_aspect_selected(mode as usize);
            let catalog_count = ResolutionCatalog::presets()
                .iter()
                .filter(|p| p.mode == mode)
                .count();
            assert_eq!(app.preset_choices.len(), catalog_count + 1);
            for choice in app.preset_choices.iter().skip(1) {
                assert!(
                    choice.label.contains(expected_ratio)
                        || (mode == 0
                            && (choice.label.contains("(4:3)") || choice.label.contains("(5:4)"))),
                    "unexpected label {}",
                    choice.label
                );
            }
            assert!(app.preset_choices[app.selected_preset]
                .label
                .starts_with(expected_first));
            assert_eq!(app.custom_width, expected_first.split('x').next().unwrap());
        }
    }

    #[test]
    fn supported_display_modes_are_listed_before_catalog_only_modes() {
        let mut app = ResEditApp::new(None, None, None);
        app.supported_modes = [(1280, 720), (1360, 768), (1920, 1080)]
            .into_iter()
            .collect();
        app.on_aspect_selected(1);
        let labels: Vec<_> = app.preset_choices.iter().map(|c| c.label.clone()).collect();
        assert!(labels[1].contains("available"));
        assert!(labels[2].contains("available"));
        assert!(labels
            .iter()
            .any(|l| l.starts_with("1360x768") && l.contains("available")));
        assert!(labels.iter().any(|l| l.contains("not reported")));
    }

    #[test]
    fn reset_and_apply_reflect_pending_state() {
        let dir = tempfile::tempdir().expect("tempdir");
        let config = dir.path().join("cs2_video.txt");
        std::fs::write(
            &config,
            "\"setting.defaultres\" \"1920\"\n\"setting.defaultresheight\" \"1080\"\n\"setting.aspectratiomode\" \"1\"\n",
        )
        .expect("write config");
        let mut app = ResEditApp::new(None, None, None);
        app.supported_modes.clear();
        app.load_path(Some(config.clone()));
        assert!(!app.can_apply());
        assert!(!app.has_pending_changes());
        app.custom_width = "1280".to_string();
        app.pending_changed();
        assert!(app.can_apply());
        assert!(app.has_pending_changes());
        app.apply();
        assert_eq!(app.configs.read(&config).expect("read back").width, 1280);
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
        assert!(app.status_warning);
        app.custom_width = "1280".to_string();
        app.pending_changed();
        app.apply();
        app.open_backups();
        assert!(!app.backups.is_empty());
        app.selected_backup = Some(0);
        app.restore_selected_backup();
        assert!(!app.show_backups);
        assert!(app.status_warning);
        assert!(app.status.contains("Backup restored successfully."));
        assert!(app.status.contains("Preferences could not be saved"));
    }

    #[test]
    fn apply_requires_reload_after_external_file_change() {
        let dir = tempfile::tempdir().expect("tempdir");
        let config = dir.path().join("cs2_video.txt");
        let original = "\"setting.defaultres\" \"1920\"\n\"setting.defaultresheight\" \"1080\"\n\"setting.aspectratiomode\" \"1\"\n";
        std::fs::write(&config, original).expect("write config");
        let mut app = ResEditApp::new(None, None, None);
        app.load_path(Some(config.clone()));
        app.custom_width = "1280".to_string();
        app.pending_changed();
        let externally_changed = format!("{original}// external edit\n");
        std::fs::write(&config, &externally_changed).expect("external edit");
        app.apply();
        assert!(app.status_warning);
        assert!(app.status.contains("Reload"));
        assert!(app
            .external_change_notice
            .as_ref()
            .unwrap()
            .contains("other file content changed"));
        assert_eq!(
            std::fs::read_to_string(&config).unwrap(),
            externally_changed
        );
        assert!(app.configs.get_backups(&config).unwrap().is_empty());

        app.load_path(Some(config.clone()));
        assert!(app.external_change_notice.is_none());
        app.custom_width = "1280".to_string();
        app.pending_changed();
        app.apply();
        assert_eq!(app.original.unwrap().width, 1280);
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
        app.custom_width = "1280".to_string();
        app.pending_changed();
        app.adopt_accounts(0, vec![custom_account(&config)]);
        assert_eq!(app.custom_width, "1280");
        assert!(app.has_pending_changes());
        assert!(app.status.contains("Unsaved changes kept"));
    }

    #[test]
    fn failed_load_preserves_current_configuration_and_pending_changes() {
        let dir = tempfile::tempdir().expect("tempdir");
        let valid = dir.path().join("cs2_video.txt");
        let invalid = dir.path().join("invalid.txt");
        std::fs::write(
            &valid,
            "\"setting.defaultres\" \"1920\"\n\"setting.defaultresheight\" \"1080\"\n\"setting.aspectratiomode\" \"1\"\n",
        )
        .unwrap();
        std::fs::write(&invalid, "not a video configuration").unwrap();
        let mut app = ResEditApp::new(None, None, None);
        assert!(app.load_path(Some(valid.clone())));
        app.custom_width = "1280".to_string();
        app.pending_changed();

        assert!(!app.load_path(Some(invalid)));
        assert_eq!(app.selected_path.as_deref(), Some(valid.as_path()));
        assert_eq!(app.original.unwrap().width, 1920);
        assert_eq!(app.custom_width, "1280");
        assert!(app.has_pending_changes());
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

        assert_eq!(app.selected_path.as_deref(), Some(custom.as_path()));
    }

    #[test]
    fn timed_out_scan_uses_slow_polling_and_can_still_finish() {
        let mut app = ResEditApp::new(None, None, None);
        let (sender, receiver) = mpsc::channel();
        app.display_scan_seq = 7;
        app.display_scan = Some(PendingScan {
            receiver,
            started: Instant::now() - SCAN_TIMEOUT,
            timeout_reported: false,
            generation: 7,
        });

        app.poll_scans();
        assert!(!app.scans_need_fast_poll());
        assert!(app.status.contains("longer than expected"));

        sender.send(Vec::new()).unwrap();
        app.poll_scans();
        assert!(app.display_scan.is_none());
    }

    #[test]
    fn stale_scan_generations_are_dropped_and_refresh_recovers() {
        let mut app = ResEditApp::new(None, None, None);
        // Simulate a hung first-generation scan that already timed out.
        let (old_sender, old_receiver) = mpsc::channel::<Vec<DisplayInfo>>();
        app.display_scan_seq = 1;
        app.display_scan = Some(PendingScan {
            receiver: old_receiver,
            started: Instant::now() - SCAN_TIMEOUT,
            timeout_reported: true,
            generation: 1,
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
        assert_eq!(app.display_scan_seq, 2);
        assert_eq!(app.display_scan.as_ref().unwrap().generation, 2);
        // Fresh (non-timed-out) scans still block replacement.
        app.refresh_displays();
        assert_eq!(app.display_scan_seq, 2);
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
    fn restore_requires_reload_after_external_file_change() {
        let dir = tempfile::tempdir().expect("tempdir");
        let config = dir.path().join("cs2_video.txt");
        std::fs::write(
            &config,
            "\"setting.defaultres\" \"1920\"\n\"setting.defaultresheight\" \"1080\"\n\"setting.aspectratiomode\" \"1\"\n",
        )
        .unwrap();
        let mut app = ResEditApp::new(None, None, None);
        app.load_path(Some(config.clone()));
        app.custom_width = "1280".to_string();
        app.pending_changed();
        app.apply();
        app.open_backups();
        app.selected_backup = Some(0);
        let changed = std::fs::read_to_string(&config).unwrap().replace(
            "\"setting.defaultres\" \"1280\"",
            "\"setting.defaultres\" \"1600\"",
        );
        std::fs::write(&config, &changed).unwrap();
        app.restore_selected_backup();
        assert!(app.status_warning);
        assert!(app.status.contains("Reload"));
        let comparison = app.external_change_notice.as_ref().unwrap();
        assert!(comparison.contains("Loaded: 1280 × 1080"));
        assert!(comparison.contains("On disk: 1600 × 1080"));
        assert_eq!(std::fs::read_to_string(&config).unwrap(), changed);
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
    fn external_change_comparison_distinguishes_display_and_other_edits() {
        let loaded = VideoConfigState {
            width: 1920,
            height: 1080,
            aspect_mode: 1,
        };
        let changed = VideoConfigState {
            width: 1280,
            height: 960,
            aspect_mode: 0,
        };
        let display_change = describe_external_change(loaded, Ok(changed));
        assert!(display_change.contains("Loaded: 1920 × 1080"));
        assert!(display_change.contains("On disk: 1280 × 960"));
        assert!(display_change.contains("4:3"));

        let other_change = describe_external_change(loaded, Ok(loaded));
        assert!(other_change.contains("display settings are the same"));
        assert!(other_change.contains("other file content changed"));
        assert!(!other_change.contains("On disk: 1280"));

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
        assert!(app.load_warning.is_some());
        let warning = app.status.clone();
        assert!(app.status_warning);
        // Simulate the auto-select load path from adopt_accounts.
        app.selected_account = Some(0);
        app.accounts.push(custom_account(&config));
        app.load_path(Some(config));
        assert!(app.status_warning);
        assert!(app.status.contains("Configuration loaded."));
        assert!(
            app.status.contains("Preferences could not be loaded")
                || warning.contains("Preferences could not be loaded"),
            "read warning lost: {}",
            app.status
        );
        assert!(app.load_warning.is_none());
    }

    #[test]
    fn pending_refresh_preserves_operation_warnings() {
        let mut app = ResEditApp::new(None, None, None);
        app.original = Some(VideoConfigState {
            width: 1920,
            height: 1080,
            aspect_mode: 1,
        });
        app.aspect_index = 1;
        app.custom_width = "1920".to_string();
        app.custom_height = "1080".to_string();
        app.supported_modes.clear();
        app.set_status("Apply failed: disk full", true);
        app.pending_changed();
        assert!(app.status_warning);
        assert_eq!(app.status, "Apply failed: disk full");
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
        app.custom_width = "1280".to_string();
        app.pending_changed();
        app.apply();
        app.open_backups();
        app.selected_backup = Some(0);
        app.confirm_restore = true;
        // Corrupt the live file so restore fails validation.
        std::fs::write(&config, "invalid").unwrap();
        // Refresh snapshot mismatch path: external edit then restore attempt.
        app.restore_selected_backup();
        assert!(app.status_warning);
        assert!(!app.confirm_restore);
    }

    #[test]
    fn invalid_dimensions_keep_footer_operation_status() {
        let mut app = ResEditApp::new(None, None, None);
        app.original = Some(VideoConfigState {
            width: 1920,
            height: 1080,
            aspect_mode: 1,
        });
        app.aspect_index = 1;
        app.custom_width = "1920".to_string();
        app.custom_height = "1080".to_string();
        app.supported_modes.clear();
        app.set_status("Apply failed: disk full", true);
        app.custom_width = "not-a-number".to_string();
        app.pending_changed();
        assert!(matches!(app.edit, EditState::Invalid));
        assert!(!app.validation.is_empty());
        assert!(app.status_warning);
        assert_eq!(app.status, "Apply failed: disk full");
        assert!(!app.can_apply());
        assert_eq!(app.apply_blocker_reason(), Some("Enter valid dimensions"));
    }

    #[test]
    fn preview_fallback_survives_invalid_keystrokes() {
        let mut app = ResEditApp::new(None, None, None);
        app.original = Some(VideoConfigState {
            width: 1920,
            height: 1080,
            aspect_mode: 1,
        });
        app.aspect_index = 1;
        app.custom_width = "1280".to_string();
        app.custom_height = "960".to_string();
        app.supported_modes.clear();
        app.pending_changed();
        assert!(app.edit.resolution().is_some());
        let fallback = app.preview_fallback.expect("fallback");
        assert_eq!((fallback.width, fallback.height), (1280, 960));
        // Intermediate typing state: invalid, but the preview shape persists.
        app.custom_width = "12".to_string();
        app.pending_changed();
        assert!(matches!(app.edit, EditState::Invalid));
        assert_eq!(app.preview_fallback, Some(fallback));
        assert!(app.preview_fallback.unwrap().width > 0);
    }

    #[test]
    fn apply_blocker_reason_covers_all_disabled_states() {
        let mut app = ResEditApp::new(None, None, None);
        assert_eq!(app.apply_blocker_reason(), Some("Select a configuration"));
        app.original = Some(VideoConfigState {
            width: 1920,
            height: 1080,
            aspect_mode: 1,
        });
        app.aspect_index = 1;
        app.custom_width = "bogus".to_string();
        app.custom_height = "1080".to_string();
        app.supported_modes.clear();
        app.pending_changed();
        assert_eq!(app.apply_blocker_reason(), Some("Enter valid dimensions"));
        app.custom_width = "1920".to_string();
        app.pending_changed();
        assert_eq!(app.apply_blocker_reason(), Some("No changes to apply"));
        app.custom_width = "1280".to_string();
        app.pending_changed();
        assert_eq!(app.apply_blocker_reason(), None);
        assert!(app.can_apply());
    }

    #[test]
    fn edit_state_tracks_no_file_invalid_unchanged_changed() {
        let dir = tempfile::tempdir().expect("tempdir");
        let config = dir.path().join("cs2_video.txt");
        std::fs::write(
            &config,
            "\"setting.defaultres\" \"1920\"\n\"setting.defaultresheight\" \"1080\"\n\"setting.aspectratiomode\" \"1\"\n",
        )
        .unwrap();
        let mut app = ResEditApp::new(None, None, None);
        assert_eq!(app.edit, EditState::NoFile);
        assert_eq!(app.apply_blocker_reason(), Some("Select a configuration"));
        app.supported_modes.clear();
        app.load_path(Some(config));
        assert!(matches!(app.edit, EditState::Unchanged { .. }));
        assert_eq!(app.apply_blocker_reason(), Some("No changes to apply"));
        app.custom_width = "1280".to_string();
        app.pending_changed();
        assert!(matches!(app.edit, EditState::Changed { .. }));
        assert_eq!(app.apply_blocker_reason(), None);
        app.custom_width = "bogus".to_string();
        app.pending_changed();
        assert_eq!(app.edit, EditState::Invalid);
        assert_eq!(app.edit.resolution(), None);
        assert_eq!(app.apply_blocker_reason(), Some("Enter valid dimensions"));
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
}
