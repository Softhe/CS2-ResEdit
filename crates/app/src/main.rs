// GUI subsystem: no console window alongside the app window on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
#[cfg(windows)]
mod taskbar_icon;
mod theme;

use app::ResEditApp;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match parse_command(&args) {
        Command::Help => {
            show_command_text("CS2 ResEdit help", help_text());
            return;
        }
        Command::Version => {
            show_command_text(
                "CS2 ResEdit version",
                &format!("CS2 ResEdit {}", env!("CARGO_PKG_VERSION")),
            );
            return;
        }
        Command::Unsupported => {
            let message = "Supported options: --help and --version.";
            eprintln!("{message}");
            let _ = rfd::MessageDialog::new()
                .set_title("Unsupported command")
                .set_description(message)
                .set_buttons(rfd::MessageButtons::Ok)
                .show();
            std::process::exit(2);
        }
        Command::Launch => {}
    }

    let _instance_guard = match SingleInstanceGuard::acquire() {
        Ok(Some(guard)) => guard,
        Ok(None) => {
            let _ = rfd::MessageDialog::new()
                .set_title("CS2 ResEdit is already running")
                .set_description(
                    "Close the existing CS2 ResEdit window before starting another one.",
                )
                .set_buttons(rfd::MessageButtons::Ok)
                .show();
            return;
        }
        Err(err) => {
            eprintln!("Could not check whether CS2 ResEdit is already running: {err}");
            std::process::exit(1);
        }
    };

    let settings_path = std::env::var("CS2_RESEDIT_SETTINGS_PATH")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .map(PathBuf::from);
    let steam_root = std::env::var("CS2_RESEDIT_STEAM_ROOT")
        .ok()
        .filter(|v| !v.trim().is_empty());

    let icon = load_icon();
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("CS2 ResEdit")
        .with_inner_size(startup_inner_size())
        .with_resizable(false);
    if let Some(icon) = icon {
        viewport = viewport.with_icon(icon);
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    if let Err(err) = eframe::run_native(
        "CS2 ResEdit",
        options,
        Box::new(move |_cc| {
            Ok(Box::new(ResEditApp::new(
                settings_path.clone(),
                steam_root.clone(),
                None,
            )) as Box<dyn eframe::App>)
        }),
    ) {
        eprintln!("CS2 ResEdit failed to start: {err}");
        std::process::exit(1);
    }
}

fn fit_inner_size(work_width: f32, work_height: f32, dpi: f32) -> [f32; 2] {
    let scale = (dpi / 96.0).max(1.0);
    // Leave room for the non-client border and title bar inside the work area.
    [
        1200.0_f32.min(work_width / scale - 24.0),
        640.0_f32.min(work_height / scale - 64.0),
    ]
}

#[cfg(windows)]
fn startup_inner_size() -> [f32; 2] {
    use windows_sys::Win32::Foundation::RECT;
    use windows_sys::Win32::UI::HiDpi::GetDpiForSystem;
    use windows_sys::Win32::UI::WindowsAndMessaging::{SystemParametersInfoW, SPI_GETWORKAREA};

    let mut work_area = RECT::default();
    if unsafe { SystemParametersInfoW(SPI_GETWORKAREA, 0, (&mut work_area as *mut RECT).cast(), 0) }
        == 0
    {
        return [1200.0, 640.0];
    }
    let dpi = unsafe { GetDpiForSystem() } as f32;
    fit_inner_size(
        (work_area.right - work_area.left) as f32,
        (work_area.bottom - work_area.top) as f32,
        dpi,
    )
}

#[cfg(not(windows))]
fn startup_inner_size() -> [f32; 2] {
    [1200.0, 640.0]
}

#[derive(Debug, Eq, PartialEq)]
enum Command {
    Launch,
    Help,
    Version,
    Unsupported,
}

fn parse_command(args: &[String]) -> Command {
    match args {
        [] => Command::Launch,
        [arg] if arg == "--help" || arg == "-h" => Command::Help,
        [arg] if arg == "--version" || arg == "-V" => Command::Version,
        _ => Command::Unsupported,
    }
}

fn help_text() -> &'static str {
    "CS2 ResEdit: edit Counter-Strike 2 display settings.\n\nUsage: CS2-ResEdit.exe [--help | --version]\n\nOptions:\n  --help, -h       Show this help text\n  --version, -V    Show the application version"
}

fn show_command_text(title: &str, text: &str) {
    if cfg!(debug_assertions) {
        println!("{text}");
    } else {
        let _ = rfd::MessageDialog::new()
            .set_title(title)
            .set_description(text)
            .set_buttons(rfd::MessageButtons::Ok)
            .show();
    }
}

#[cfg(windows)]
struct SingleInstanceGuard(windows_sys::Win32::Foundation::HANDLE);

#[cfg(windows)]
impl SingleInstanceGuard {
    fn acquire() -> std::io::Result<Option<Self>> {
        Self::acquire_named("Local\\Softhe.CS2ResEdit.v1")
    }

    fn acquire_named(name: &str) -> std::io::Result<Option<Self>> {
        use windows_sys::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
        use windows_sys::Win32::System::Threading::CreateMutexW;

        let name: Vec<u16> = format!("{name}\0").encode_utf16().collect();
        // The named mutex is owned by the OS and released if the process exits.
        let handle = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
        if handle.is_null() {
            return Err(std::io::Error::last_os_error());
        }
        if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
            unsafe { windows_sys::Win32::Foundation::CloseHandle(handle) };
            return Ok(None);
        }
        Ok(Some(Self(handle)))
    }
}

#[cfg(windows)]
impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        unsafe { windows_sys::Win32::Foundation::CloseHandle(self.0) };
    }
}

#[cfg(not(windows))]
struct SingleInstanceGuard;

#[cfg(not(windows))]
impl SingleInstanceGuard {
    fn acquire() -> std::io::Result<Option<Self>> {
        Ok(Some(Self))
    }
}

fn load_icon() -> Option<egui::IconData> {
    theme::ThemePalette::for_theme(cs2_resedit_core::UiTheme::Glacier)
        .window_icon()
        .map(|icon| (*icon).clone())
}

#[cfg(test)]
mod tests {
    use super::{fit_inner_size, parse_command, Command};

    #[test]
    fn startup_size_fits_1280_by_960_work_area_at_common_dpi() {
        assert_eq!(fit_inner_size(1280.0, 912.0, 96.0), [1200.0, 640.0]);
        for dpi in [96.0, 120.0, 144.0] {
            let [width, height] = fit_inner_size(1280.0, 912.0, dpi);
            assert!((width + 24.0) * dpi / 96.0 <= 1280.0);
            assert!((height + 64.0) * dpi / 96.0 <= 912.0);
        }
    }

    #[test]
    fn command_line_accepts_only_help_and_version() {
        assert_eq!(parse_command(&[]), Command::Launch);
        for option in ["--help", "-h"] {
            assert_eq!(parse_command(&[option.into()]), Command::Help);
        }
        for option in ["--version", "-V"] {
            assert_eq!(parse_command(&[option.into()]), Command::Version);
        }
        assert_eq!(parse_command(&["--unknown".into()]), Command::Unsupported);
        assert_eq!(
            parse_command(&["--help".into(), "extra".into()]),
            Command::Unsupported
        );
    }

    #[cfg(windows)]
    #[test]
    fn single_instance_guard_rejects_a_second_owner_and_releases_on_drop() {
        let name = format!("Local\\Softhe.CS2ResEdit.test.{}", std::process::id());
        let first = super::SingleInstanceGuard::acquire_named(&name)
            .expect("acquire mutex")
            .expect("first owner");
        assert!(super::SingleInstanceGuard::acquire_named(&name)
            .expect("check second owner")
            .is_none());
        drop(first);
        assert!(super::SingleInstanceGuard::acquire_named(&name)
            .expect("reacquire mutex")
            .is_some());
    }
}
