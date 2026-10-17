//! Small Windows system probes: whether CS2 is running and whether Windows
//! High Contrast is on. Other platforms report neither.

/// Whether a process named `cs2.exe` is running. `None` when the process
/// list cannot be read; callers then proceed without a warning.
#[cfg(windows)]
pub fn game_running() -> Option<bool> {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };

    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot == INVALID_HANDLE_VALUE {
        return None;
    }
    let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..unsafe { std::mem::zeroed() }
    };
    let mut found = false;
    // The snapshot handle is valid and `entry.dwSize` is initialized.
    let mut more = unsafe { Process32FirstW(snapshot, &mut entry) } != 0;
    while more {
        let end = entry
            .szExeFile
            .iter()
            .position(|&unit| unit == 0)
            .unwrap_or(entry.szExeFile.len());
        if String::from_utf16_lossy(&entry.szExeFile[..end]).eq_ignore_ascii_case("cs2.exe") {
            found = true;
            break;
        }
        more = unsafe { Process32NextW(snapshot, &mut entry) } != 0;
    }
    unsafe { CloseHandle(snapshot) };
    Some(found)
}

#[cfg(not(windows))]
pub fn game_running() -> Option<bool> {
    None
}

/// Whether Windows High Contrast is currently on.
#[cfg(windows)]
pub fn high_contrast_enabled() -> bool {
    use windows_sys::Win32::UI::Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTW};
    use windows_sys::Win32::UI::WindowsAndMessaging::{SystemParametersInfoW, SPI_GETHIGHCONTRAST};

    let mut settings = HIGHCONTRASTW {
        cbSize: std::mem::size_of::<HIGHCONTRASTW>() as u32,
        dwFlags: 0,
        lpszDefaultScheme: std::ptr::null_mut(),
    };
    // The struct is sized and outlives the call.
    let ok = unsafe {
        SystemParametersInfoW(
            SPI_GETHIGHCONTRAST,
            settings.cbSize,
            (&mut settings as *mut HIGHCONTRASTW).cast(),
            0,
        )
    } != 0;
    ok && settings.dwFlags & HCF_HIGHCONTRASTON != 0
}

#[cfg(not(windows))]
pub fn high_contrast_enabled() -> bool {
    false
}

#[cfg(all(test, windows))]
mod tests {
    #[test]
    fn probes_answer_without_panicking() {
        assert!(super::game_running().is_some());
        let _ = super::high_contrast_enabled();
    }
}
