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
const ATTACHED_TO_DESKTOP: u32 = 0x1;
#[cfg(windows)]
const PRIMARY_DEVICE: u32 = 0x4;
#[cfg(windows)]
const CURRENT_SETTINGS: i32 = -1;

// Compile-time guards: the FFI structs must match the native
// DISPLAY_DEVICEW (840 bytes) and DEVMODEW (220 bytes) layouts exactly.
#[cfg(windows)]
const _: () = assert!(std::mem::size_of::<DisplayDeviceW>() == 840);
#[cfg(windows)]
const _: () = assert!(std::mem::size_of::<DevModeW>() == 220);
#[cfg(windows)]
const _: () = assert!(std::mem::align_of::<DisplayDeviceW>() == 4);
#[cfg(windows)]
const _: () = assert!(std::mem::align_of::<DevModeW>() == 4);

#[cfg(windows)]
#[repr(C)]
struct DisplayDeviceW {
    cb: u32,
    device_name: [u16; 32],
    device_string: [u16; 128],
    state_flags: u32,
    _device_id: [u16; 128],
    _device_key: [u16; 128],
}

#[cfg(windows)]
#[repr(C)]
struct DevModeW {
    device_name: [u16; 32],
    spec_version: u16,
    driver_version: u16,
    size: u16,
    driver_extra: u16,
    _fields: u32,
    // Union: dmPosition (POINTL: 2x i32) + display orientation/fixed output (2x u32)
    _union_a: [u32; 4],
    _color_duplex_yres_ttoption_collate: [u16; 5],
    _form_name: [u16; 32],
    _log_pixels: u16,
    bits_per_pel: u32,
    pels_width: u32,
    pels_height: u32,
    _display_flags_or_nup: u32,
    _display_frequency: u32,
    _icm_method: u32,
    _icm_intent: u32,
    _media_type: u32,
    _dither_type: u32,
    _reserved1: u32,
    _reserved2: u32,
    _panning_width: u32,
    _panning_height: u32,
}

#[cfg(windows)]
impl DevModeW {
    fn blank() -> Self {
        Self {
            device_name: [0; 32],
            spec_version: 0,
            driver_version: 0,
            size: std::mem::size_of::<Self>() as u16,
            driver_extra: 0,
            _fields: 0,
            _union_a: [0; 4],
            _color_duplex_yres_ttoption_collate: [0; 5],
            _form_name: [0; 32],
            _log_pixels: 0,
            bits_per_pel: 0,
            pels_width: 0,
            pels_height: 0,
            _display_flags_or_nup: 0,
            _display_frequency: 0,
            _icm_method: 0,
            _icm_intent: 0,
            _media_type: 0,
            _dither_type: 0,
            _reserved1: 0,
            _reserved2: 0,
            _panning_width: 0,
            _panning_height: 0,
        }
    }
}

#[cfg(windows)]
#[link(name = "user32")]
extern "system" {
    fn EnumDisplayDevicesW(
        lp_device: *const u16,
        i_dev_num: u32,
        lp_display_device: *mut DisplayDeviceW,
        dw_flags: u32,
    ) -> i32;
    fn EnumDisplaySettingsW(
        lpsz_device_name: *const u16,
        i_mode_num: i32,
        lp_dev_mode: *mut DevModeW,
    ) -> i32;
}

#[cfg(windows)]
fn wide_to_string(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

#[cfg(windows)]
fn enumerate_windows_displays() -> Vec<DisplayInfo> {
    let monitor_names = active_monitor_names();
    let mut displays: Vec<DisplayInfo> = Vec::new();
    let mut index: u32 = 0;
    loop {
        let mut device = DisplayDeviceW {
            cb: std::mem::size_of::<DisplayDeviceW>() as u32,
            device_name: [0; 32],
            device_string: [0; 128],
            state_flags: 0,
            _device_id: [0; 128],
            _device_key: [0; 128],
        };
        let ok =
            unsafe { EnumDisplayDevicesW(std::ptr::null(), index, &mut device as *mut _, 0) } != 0;
        if !ok {
            break;
        }
        index += 1;
        if (device.state_flags & ATTACHED_TO_DESKTOP) == 0 {
            continue;
        }

        let device_name = wide_to_string(&device.device_name);
        let friendly = wide_to_string(&device.device_string);
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
        let mut name_nul: Vec<u16> = device.device_name.to_vec();
        if !name_nul.contains(&0) {
            name_nul.push(0);
        }

        let mut modes_set = std::collections::BTreeSet::new();
        let mut mode_index: i32 = 0;
        loop {
            let mut mode = DevModeW::blank();
            let ok =
                unsafe { EnumDisplaySettingsW(name_nul.as_ptr(), mode_index, &mut mode as *mut _) }
                    != 0;
            if !ok {
                break;
            }
            mode_index += 1;
            if mode.pels_width >= 320 && mode.pels_height >= 200 {
                modes_set.insert((mode.pels_width as i32, mode.pels_height as i32));
            }
        }

        let mut current = DevModeW::blank();
        let has_current = unsafe {
            EnumDisplaySettingsW(name_nul.as_ptr(), CURRENT_SETTINGS, &mut current as *mut _)
        } != 0;

        displays.push(DisplayInfo {
            device_name,
            friendly_name: friendly,
            is_primary: (device.state_flags & PRIMARY_DEVICE) != 0,
            current_width: if has_current {
                current.pels_width as i32
            } else {
                0
            },
            current_height: if has_current {
                current.pels_height as i32
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
