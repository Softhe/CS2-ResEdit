use crate::models::DisplayInfo;
#[cfg(windows)]
use crate::models::DisplayResolution;

pub trait DisplayModeProvider: Send + Sync {
    fn get_displays(&self) -> Vec<DisplayInfo>;
}

/// Windows display enumeration uses GDI for modes and the display configuration
/// API for monitor model names; elsewhere it returns an empty list.
#[derive(Clone, Copy, Default)]
pub struct DisplayProvider;

impl DisplayModeProvider for DisplayProvider {
    #[cfg(windows)]
    fn get_displays(&self) -> Vec<DisplayInfo> {
        enumerate_windows_displays()
    }

    #[cfg(not(windows))]
    fn get_displays(&self) -> Vec<DisplayInfo> {
        Vec::new()
    }
}

#[cfg(windows)]
fn wide_to_string(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

#[cfg(windows)]
fn enumerate_windows_displays() -> Vec<DisplayInfo> {
    use windows_sys::Win32::Graphics::Gdi::{
        EnumDisplayDevicesW, EnumDisplaySettingsW, DEVMODEW, DISPLAY_DEVICEW,
        DISPLAY_DEVICE_ATTACHED_TO_DESKTOP, DISPLAY_DEVICE_PRIMARY_DEVICE, ENUM_CURRENT_SETTINGS,
    };

    fn blank_mode() -> DEVMODEW {
        DEVMODEW {
            dmSize: std::mem::size_of::<DEVMODEW>() as u16,
            ..DEVMODEW::default()
        }
    }

    let monitor_names = active_monitor_names();
    let mut displays: Vec<DisplayInfo> = Vec::new();
    let mut index: u32 = 0;
    loop {
        let mut device = DISPLAY_DEVICEW {
            cb: std::mem::size_of::<DISPLAY_DEVICEW>() as u32,
            ..DISPLAY_DEVICEW::default()
        };
        // `device.cb` is initialized; a null adapter name enumerates adapters.
        let ok = unsafe { EnumDisplayDevicesW(std::ptr::null(), index, &mut device, 0) } != 0;
        if !ok {
            break;
        }
        index += 1;
        if (device.StateFlags & DISPLAY_DEVICE_ATTACHED_TO_DESKTOP) == 0 {
            continue;
        }

        let device_name = wide_to_string(&device.DeviceName);
        let friendly = wide_to_string(&device.DeviceString);
        let friendly = if friendly.trim().is_empty() {
            format!("Display {}", displays.len() + 1)
        } else {
            friendly
        };
        let friendly = monitor_names
            .get(&device_name)
            .map(|names| names.join(" / "))
            .unwrap_or(friendly);
        // NUL-terminated device name for the settings query.
        let mut name_nul: Vec<u16> = device.DeviceName.to_vec();
        if !name_nul.contains(&0) {
            name_nul.push(0);
        }

        let mut modes_set = std::collections::BTreeSet::new();
        let mut mode_index: u32 = 0;
        loop {
            let mut mode = blank_mode();
            // `name_nul` is NUL-terminated and `mode.dmSize` is initialized.
            let ok = unsafe { EnumDisplaySettingsW(name_nul.as_ptr(), mode_index, &mut mode) } != 0;
            if !ok {
                break;
            }
            mode_index += 1;
            if mode.dmPelsWidth >= 320 && mode.dmPelsHeight >= 200 {
                modes_set.insert((mode.dmPelsWidth as i32, mode.dmPelsHeight as i32));
            }
        }

        let mut current = blank_mode();
        let has_current =
            unsafe { EnumDisplaySettingsW(name_nul.as_ptr(), ENUM_CURRENT_SETTINGS, &mut current) }
                != 0;

        displays.push(DisplayInfo {
            device_name,
            friendly_name: friendly,
            is_primary: (device.StateFlags & DISPLAY_DEVICE_PRIMARY_DEVICE) != 0,
            current_width: if has_current {
                current.dmPelsWidth as i32
            } else {
                0
            },
            current_height: if has_current {
                current.dmPelsHeight as i32
            } else {
                0
            },
            modes: modes_set
                .into_iter()
                .map(|(w, h)| DisplayResolution::new(w, h))
                .collect(),
        });
    }
    displays.sort_by(|a, b| {
        b.is_primary
            .cmp(&a.is_primary)
            .then(a.friendly_name.cmp(&b.friendly_name))
    });
    displays
}

/// Match each active monitor's EDID-friendly name to its GDI display source.
/// Clone-mode sources may drive multiple monitors; retain every distinct name.
#[cfg(windows)]
fn active_monitor_names() -> std::collections::HashMap<String, Vec<String>> {
    use windows_sys::Win32::Devices::Display::*;
    let mut result = std::collections::HashMap::<String, Vec<String>>::new();
    // A monitor can be connected between the size query and the data query.
    for _ in 0..3 {
        let (mut path_count, mut mode_count) = (0, 0);
        if unsafe {
            GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut path_count, &mut mode_count)
        } != 0
        {
            return result;
        }
        let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); path_count as usize];
        let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); mode_count as usize];
        let status = unsafe {
            QueryDisplayConfig(
                QDC_ONLY_ACTIVE_PATHS,
                &mut path_count,
                paths.as_mut_ptr(),
                &mut mode_count,
                modes.as_mut_ptr(),
                std::ptr::null_mut(),
            )
        };
        if status == 122 {
            continue;
        }
        if status != 0 {
            return result;
        }
        for path in paths.into_iter().take(path_count as usize) {
            let mut source = DISPLAYCONFIG_SOURCE_DEVICE_NAME::default();
            source.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME;
            source.header.size = std::mem::size_of_val(&source) as u32;
            source.header.adapterId = path.sourceInfo.adapterId;
            source.header.id = path.sourceInfo.id;
            let mut target = DISPLAYCONFIG_TARGET_DEVICE_NAME::default();
            target.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME;
            target.header.size = std::mem::size_of_val(&target) as u32;
            target.header.adapterId = path.targetInfo.adapterId;
            target.header.id = path.targetInfo.id;
            // Both packets have initialized headers with the SDK-defined sizes.
            if unsafe { DisplayConfigGetDeviceInfo(&mut source.header) } != 0
                || unsafe { DisplayConfigGetDeviceInfo(&mut target.header) } != 0
            {
                continue;
            }
            let name = wide_to_string(&target.monitorFriendlyDeviceName);
            if !name.trim().is_empty() {
                let names = result
                    .entry(wide_to_string(&source.viewGdiDeviceName))
                    .or_default();
                if !names.contains(&name) {
                    names.push(name);
                }
            }
        }
        break;
    }
    result
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires a live Windows desktop"]
    fn live_monitor_names_are_mapped_to_display_sources() {
        let names = active_monitor_names();
        assert!(
            !names.is_empty(),
            "Windows returned no active monitor names"
        );
        let displays = enumerate_windows_displays();
        for display in displays {
            if let Some(names) = names.get(&display.device_name) {
                assert_eq!(display.friendly_name, names.join(" / "));
                println!("{}: {}", display.device_name, display.friendly_name);
            }
        }
    }
}
