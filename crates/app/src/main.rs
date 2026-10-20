// GUI subsystem: no console window alongside the app window on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod system;
#[cfg(windows)]
mod taskbar_icon;
mod theme;

use app::ResEditApp;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

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
        Err(err) => fail_startup(&format!(
            "Could not check whether CS2 ResEdit is already running: {err}"
        )),
    };

    let settings_path = std::env::var("CS2_RESEDIT_SETTINGS_PATH")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .map(PathBuf::from);
    let steam_root = std::env::var("CS2_RESEDIT_STEAM_ROOT")
        .ok()
        .filter(|v| !v.trim().is_empty());

    let launch = |renderer: Renderer, started: &AtomicBool| {
        let settings_path = settings_path.clone();
        let steam_root = steam_root.clone();
        eframe::run_native(
            "CS2 ResEdit",
            native_options(renderer),
            Box::new(move |_cc| {
                started.store(true, Ordering::Relaxed);
                Ok(Box::new(ResEditApp::new(settings_path, steam_root, None))
                    as Box<dyn eframe::App>)
            }),
        )
        .map_err(|err| (renderer, err.to_string()))
    };
    let setting = std::env::var_os("CS2_RESEDIT_RENDERER");
    let setting = setting.as_ref().map(|value| value.to_string_lossy());
    let renderers =
        requested_renderers(setting.as_deref()).unwrap_or_else(|message| fail_startup(&message));
    if let Err(failures) = run_with_fallback(renderers, launch) {
        fail_startup(&startup_failure_text(&failures));
    }
}

/// Graphics back ends the window can be drawn with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Renderer {
    OpenGl,
    #[cfg(windows)]
    Direct3D12,
}

impl Renderer {
    fn label(self) -> &'static str {
        match self {
            Self::OpenGl => "OpenGL",
            #[cfg(windows)]
            Self::Direct3D12 => "Direct3D 12",
        }
    }
}

/// Renderers to try in order for a `CS2_RESEDIT_RENDERER` value. OpenGL is
/// the tested default; Direct3D 12 covers machines without a usable OpenGL
/// driver, such as some virtual machines and Remote Desktop sessions. An
/// unset value tries both; `opengl` or `d3d12` pins one. Anything else is an
/// error, so a typo cannot silently select a different renderer.
fn requested_renderers(setting: Option<&str>) -> Result<Vec<Renderer>, String> {
    match setting
        .map(|value| value.trim().to_ascii_lowercase())
        .as_deref()
    {
        None => Ok(vec![
            Renderer::OpenGl,
            #[cfg(windows)]
            Renderer::Direct3D12,
        ]),
        Some("opengl" | "gl" | "glow") => Ok(vec![Renderer::OpenGl]),
        #[cfg(windows)]
        Some("d3d12" | "dx12" | "wgpu") => Ok(vec![Renderer::Direct3D12]),
        _ => Err(format!(
            "CS2_RESEDIT_RENDERER is set to '{}', which is not a supported renderer. \
             Use {}, or remove the variable to choose automatically.",
            setting.unwrap_or_default().trim(),
            if cfg!(windows) {
                "opengl or d3d12"
            } else {
                "opengl"
            }
        )),
    }
}

/// Runs `launch` with each renderer until one succeeds. A renderer that
/// already created the app is not retried with another one: its failure
/// happened mid-session, and reopening the window would be surprising.
fn run_with_fallback(
    renderers: Vec<Renderer>,
    mut launch: impl FnMut(Renderer, &AtomicBool) -> Result<(), (Renderer, String)>,
) -> Result<(), Vec<(Renderer, String)>> {
    let mut failures = Vec::new();
    for renderer in renderers {
        let started = AtomicBool::new(false);
        match launch(renderer, &started) {
            Ok(()) => return Ok(()),
            Err(failure) => {
                failures.push(failure);
                if started.load(Ordering::Relaxed) {
                    break;
                }
            }
        }
    }
    Err(failures)
}

fn native_options(renderer: Renderer) -> eframe::NativeOptions {
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("CS2 ResEdit")
        .with_inner_size(startup_inner_size())
        .with_resizable(false);
    if let Some(icon) = load_icon() {
        viewport = viewport.with_icon(icon);
    }
    eframe::NativeOptions {
        viewport,
        renderer: match renderer {
            Renderer::OpenGl => eframe::Renderer::Glow,
            #[cfg(windows)]
            Renderer::Direct3D12 => eframe::Renderer::Wgpu,
        },
        ..Default::default()
    }
}

fn startup_failure_text(failures: &[(Renderer, String)]) -> String {
    let mut text = String::from("CS2 ResEdit could not open its window.\n");
    for (renderer, error) in failures {
        text.push_str(&format!("\n{}: {error}", renderer.label()));
    }
    text.push_str(
        "\n\nUpdate the graphics driver and try again. Remote Desktop sessions and \
         virtual machines without graphics acceleration may not support the window.",
    );
    text
}

/// Reports a fatal startup error and exits. The release build has no
/// console, so the message must appear in a dialog to be seen at all.
fn fail_startup(message: &str) -> ! {
    eprintln!("{message}");
    if !cfg!(debug_assertions) {
        let _ = rfd::MessageDialog::new()
            .set_title("CS2 ResEdit could not start")
            .set_description(message)
            .set_level(rfd::MessageLevel::Error)
            .set_buttons(rfd::MessageButtons::Ok)
            .show();
    }
    std::process::exit(1);
}

#[cfg(any(windows, test))]
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
    use super::{
        fit_inner_size, parse_command, run_with_fallback, startup_failure_text, Command, Renderer,
    };
    use std::sync::atomic::Ordering;

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

    #[test]
    fn renderer_setting_accepts_known_names_and_rejects_typos() {
        let automatic = super::requested_renderers(None).unwrap();
        assert_eq!(automatic[0], Renderer::OpenGl);
        assert!(super::requested_renderers(Some("")).is_err());
        assert!(super::requested_renderers(Some("  ")).is_err());
        assert_eq!(
            super::requested_renderers(Some(" OpenGL ")).unwrap(),
            vec![Renderer::OpenGl]
        );
        #[cfg(windows)]
        {
            assert_eq!(automatic, vec![Renderer::OpenGl, Renderer::Direct3D12]);
            assert_eq!(
                super::requested_renderers(Some("D3D12")).unwrap(),
                vec![Renderer::Direct3D12]
            );
        }
        let error = super::requested_renderers(Some("d3d21")).unwrap_err();
        assert!(error.contains("'d3d21'"), "{error}");
    }

    #[test]
    fn renderer_fallback_stops_at_success_or_mid_session_failure() {
        // An explicit list: the result must not depend on the environment.
        let renderers = vec![
            Renderer::OpenGl,
            #[cfg(windows)]
            Renderer::Direct3D12,
        ];

        // Every renderer fails before the app exists: all are tried.
        let mut tried = Vec::new();
        let failures = run_with_fallback(renderers.clone(), |renderer, _| {
            tried.push(renderer);
            Err((renderer, "no device".into()))
        })
        .unwrap_err();
        assert_eq!(tried, renderers);
        assert_eq!(failures.len(), renderers.len());
        let text = startup_failure_text(&failures);
        assert!(text.contains("OpenGL: no device"), "{text}");
        assert!(text.contains("graphics driver"));

        // The first renderer works: nothing else runs.
        let mut calls = 0;
        assert!(run_with_fallback(renderers.clone(), |_, _| {
            calls += 1;
            Ok(())
        })
        .is_ok());
        assert_eq!(calls, 1);

        // A failure after the app started is reported, not retried.
        let mut calls = 0;
        let failures = run_with_fallback(renderers, |renderer, started| {
            calls += 1;
            started.store(true, Ordering::Relaxed);
            Err((renderer, "lost device".into()))
        })
        .unwrap_err();
        assert_eq!((calls, failures.len()), (1, 1));
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
