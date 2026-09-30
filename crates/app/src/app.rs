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

mod panels;
#[cfg(test)]
use panels::{preview_rectangle, preview_viewport};

const ASPECT_LABELS: [&str; 3] = ["4:3 / 5:4", "16:9", "16:10"];
const DISPLAY_POLL_INTERVAL: Duration = Duration::from_secs(2);
const SLOW_SCAN_THRESHOLD: Duration = Duration::from_millis(150);
const SCAN_NOTICE_DURATION: Duration = Duration::from_secs(4);
const SCAN_TIMEOUT: Duration = Duration::from_secs(10);

struct PendingScan<T> {
    receiver: mpsc::Receiver<T>,
    started: Instant,
    timeout_reported: bool,
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
    pending: Option<Resolution>,
    pending_differs: Option<bool>,
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
    display_labels: Vec<String>,
    initialized: bool,
    theme_applied: bool,
    #[cfg(windows)]
    taskbar_icon: crate::taskbar_icon::TaskbarIcon,
    last_display_poll: Instant,
    account_scan: Option<PendingScan<AccountScanResult>>,
    display_scan: Option<PendingScan<Vec<DisplayInfo>>>,
    scan_notice: Option<(String, Instant)>,
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
            pending: None,
            pending_differs: None,
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
            display_labels: Vec::new(),
            initialized: false,
            theme_applied: false,
            #[cfg(windows)]
            taskbar_icon: crate::taskbar_icon::TaskbarIcon::default(),
            last_display_poll: Instant::now(),
            account_scan: None,
            display_scan: None,
            scan_notice: None,
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
        if let Some(warning) = self.preferences.warning.clone() {
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
        if self.account_scan.is_some() {
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

    fn adopt_accounts(&mut self, root_count: usize, discovered: Vec<SteamAccount>) {
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
            self.set_status(
                "No Steam accounts found. Use Browse to select cs2_video.txt.",
                false,
            );
        } else {
            self.set_status("Steam accounts refreshed.", false);
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
                    self.set_status("Steam accounts refreshed. Unsaved changes kept.", false);
                } else {
                    if !self.load_path(Some(PathBuf::from(&account.config_path))) {
                        self.selected_account = prior_path.as_ref().and_then(|prior| {
                            self.accounts.iter().position(|entry| {
                                fsutil::paths_equal_ci(&entry.config_path, &prior.to_string_lossy())
                            })
                        });
                    }
                }
            } else {
                self.load_path(None);
                self.set_status(
                    "This account does not have a CS2 video configuration.",
                    true,
                );
            }
        } else {
            self.load_path(None);
        }
    }

    fn load_path(&mut self, path: Option<PathBuf>) -> bool {
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
                    self.set_status(&format!("Configuration loaded. {warning}"), true);
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

    fn reset_pending(&mut self) {
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

    fn on_preset_selected(&mut self, index: usize) {
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

    fn on_aspect_selected(&mut self, index: usize) {
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

    fn refresh_displays(&mut self) {
        if self.display_scan.is_some() {
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
                    self.account_scan = None;
                    self.adopt_accounts(root_count, accounts);
                    self.note_scan("Steam account scan", elapsed);
                    if !warnings.is_empty() {
                        self.set_status("Steam accounts were found, but account names could not be read from one or more Steam installations.", true);
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.account_scan = None;
                    self.set_status("Steam account scan failed.", true);
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
                    self.set_status("Display scan failed.", true);
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

    fn display_changed(&mut self) {
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

    fn populate_preset_choices(&mut self, mode: i32, width: Option<i32>, height: Option<i32>) {
        let mut supported: Vec<Resolution> = self
            .supported_modes
            .iter()
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

    fn pending_resolution(&self) -> Result<Resolution, String> {
        ResolutionCatalog::parse(
            &format!("{}x{}", self.custom_width, self.custom_height),
            Some(self.aspect_index as i32),
        )
        .map_err(|e| e.to_string())
    }

    fn pending_changed(&mut self) {
        let Some(original) = self.original else {
            self.pending = None;
            self.pending_differs = None;
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
                let differs = original
                    != cs2_resedit_core::VideoConfigState {
                        width: resolution.width,
                        height: resolution.height,
                        aspect_mode: resolution.mode,
                    };
                let name = ResolutionCatalog::mode_name(
                    resolution.mode,
                    resolution.width,
                    resolution.height,
                )
                .unwrap_or("?");
                self.pending = Some(resolution);
                self.pending_differs = Some(differs);
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
                self.status = if differs {
                    "Review the pending settings, then apply the change.".to_string()
                } else {
                    "No pending changes.".to_string()
                };
                self.status_warning = false;
            }
            Err(message) => {
                self.pending = None;
                self.pending_differs = None;
                self.pending_label = "Pending: invalid custom resolution".to_string();
                self.status = message.clone();
                self.status_warning = true;
                self.validation = message;
                self.availability.clear();
                self.availability_ok = None;
            }
        }
    }

    fn has_pending_changes(&self) -> bool {
        let Some(_) = self.original else {
            return false;
        };
        match self.pending {
            // Invalid custom dimensions keep Reset enabled (C# parity).
            None => true,
            Some(_) => self.pending_differs == Some(true),
        }
    }

    fn can_apply(&self) -> bool {
        self.pending.is_some() && self.pending_differs == Some(true)
    }

    fn apply(&mut self) {
        let Some(path) = self.selected_path.clone() else {
            return;
        };
        let Some(resolution) = self.pending else {
            return;
        };
        let Some(snapshot) = self.snapshot.as_ref() else {
            self.set_status("Reload the configuration before applying changes.", true);
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
                        self.set_status(
                            &err.context("Verifying the updated configuration failed")
                                .to_string(),
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
                            self.set_status(&format!("Changes applied. Backup: {name}"), false);
                        }
                        None => self.set_status("Changes applied.", false),
                    }
                } else {
                    self.set_status("The configuration already has these settings.", false);
                }
            }
            Err(CoreError::ExternalChange(message)) => {
                self.external_change_notice = Some(describe_external_change(
                    snapshot.state,
                    self.configs.read(&path),
                ));
                self.set_status(&message, true);
            }
            Err(err) => self.set_status(&err.to_string(), true),
        }
    }

    fn open_backups(&mut self) {
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

    fn restore_selected_backup(&mut self) {
        let (Some(config), Some(index)) = (self.selected_path.clone(), self.selected_backup) else {
            return;
        };
        let Some(backup) = self.backups.get(index).cloned() else {
            return;
        };
        let Some(snapshot) = self.snapshot.as_ref() else {
            self.set_status("Reload the configuration before restoring a backup.", true);
            return;
        };
        match self
            .configs
            .restore_if_unchanged(&config, Path::new(&backup.path), snapshot)
        {
            Ok(_) => {
                self.show_backups = false;
                self.confirm_restore = false;
                self.load_path(Some(config));
                if self.original.is_some() {
                    if let Some(warning) = self.preferences.warning.clone() {
                        self.set_status(&format!("Backup restored successfully. {warning}"), true);
                    } else {
                        self.set_status("Backup restored successfully.", false);
                    }
                }
            }
            Err(CoreError::ExternalChange(message)) => {
                self.external_change_notice = Some(describe_external_change(
                    snapshot.state,
                    self.configs.read(&config),
                ));
                self.set_status(&message, true);
            }
            Err(err) => self.set_status(&err.to_string(), true),
        }
    }

    fn open_diagnostics(&mut self) {
        let version = env!("CARGO_PKG_VERSION");
        let report = DiagnosticService::create(
            &self.configs,
            version,
            &self.displays,
            self.discovered_root_count,
            self.accounts.len(),
            self.selected_path.as_deref(),
        );
        self.diagnostics_summary = DiagnosticService::to_summary(&report);
        self.diagnostics_json = DiagnosticService::to_json(&report);
        self.show_diagnostics = true;
    }

    fn browse(&mut self) {
        let Some(file) = rfd::FileDialog::new()
            .set_title("Select the CS2 video configuration")
            .add_filter("CS2 video configuration (cs2_video.txt)", &["txt"])
            .add_filter("Text files (*.txt)", &["txt"])
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
        let palette = self.palette;

        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.heading("CS2 ResEdit");
                    ui.label("Tune your display settings safely");
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let previous = self.preferences.theme;
                    ui.add_enabled_ui(!modal_open && !palette.high_contrast, |ui| {
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
                        let current = UiTheme::ALL
                            .iter()
                            .position(|theme| *theme == self.preferences.theme)
                            .unwrap_or(0);
                        if let Some(index) =
                            panels::wheel_selection(&response.response, current, UiTheme::ALL.len())
                        {
                            self.preferences.theme = UiTheme::ALL[index];
                        }
                    });
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
                    ui.label(
                        egui::RichText::new("CS2  •  DISPLAY")
                            .color(palette.accent)
                            .strong(),
                    );
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
                    } else {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            for (index, backup) in self.backups.iter().enumerate() {
                                let name = ResolutionCatalog::mode_name(
                                    backup.state.aspect_mode,
                                    backup.state.width,
                                    backup.state.height,
                                )
                                .unwrap_or("?");
                                let selected = self.selected_backup == Some(index);
                                if ui
                                    .selectable_label(
                                        selected,
                                        format!(
                                            "{}  ·  {} × {}  ·  {name}",
                                            backup.path, backup.state.width, backup.state.height
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
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::multiline(&mut self.diagnostics_summary.as_str())
                                .desired_width(f32::INFINITY)
                                .desired_rows(12),
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
    std::fs::write(path, json.as_bytes())
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
        app.pending = Some(Resolution::new(1280, 960, "4:3", 0));
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
        app.display_scan = Some(PendingScan {
            receiver,
            started: Instant::now() - SCAN_TIMEOUT,
            timeout_reported: false,
        });

        app.poll_scans();
        assert!(!app.scans_need_fast_poll());
        assert!(app.status.contains("longer than expected"));

        sender.send(Vec::new()).unwrap();
        app.poll_scans();
        assert!(app.display_scan.is_none());
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
            .filter(|c| !c.is_ascii())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .filter(|c| !COVERED.contains(c))
            .collect();
        assert!(missing.is_empty(), "uncovered UI glyphs: {missing:?}");
    }
}
