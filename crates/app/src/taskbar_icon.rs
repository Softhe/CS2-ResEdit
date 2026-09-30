//! Windows has separate small title-bar and large taskbar icon slots.

use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateIcon, DestroyIcon, IsWindow, SendMessageW, HICON, ICON_BIG, WM_GETICON, WM_SETICON,
};

#[derive(Default)]
pub struct TaskbarIcon {
    window: HWND,
    icon: HICON,
}

impl TaskbarIcon {
    pub fn update(&mut self, frame: &eframe::Frame, icon: &egui::IconData) -> Result<(), String> {
        let handle = frame.window_handle().map_err(|error| error.to_string())?;
        let RawWindowHandle::Win32(handle) = handle.as_raw() else {
            return Err("The native window is not a Windows window.".into());
        };
        self.set(handle.hwnd.get() as HWND, icon)
            .map_err(|error| error.to_string())
    }

    fn set(&mut self, window: HWND, icon: &egui::IconData) -> std::io::Result<()> {
        let width = usize::try_from(icon.width).unwrap_or(0);
        let height = usize::try_from(icon.height).unwrap_or(0);
        if width == 0
            || height == 0
            || width > 256
            || height > 256
            || icon.rgba.len() != width * height * 4
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Invalid icon dimensions or pixel data.",
            ));
        }
        let mut bgra = icon.rgba.clone();
        // Monochrome mask rows must be aligned to WORD boundaries.
        let stride = width.div_ceil(16) * 2;
        let mut mask = vec![0_u8; stride * height];
        for (index, pixel) in bgra.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            pixel.swap(0, 2);
            if pixel[3] == 0 {
                mask[(index / width) * stride + (index % width) / 8] |=
                    0x80 >> ((index % width) % 8);
            }
        }
        // Buffers contain width*height BGRA pixels and a padded 1-bit AND mask.
        let new_icon = unsafe {
            CreateIcon(
                std::ptr::null_mut(),
                width as i32,
                height as i32,
                1,
                32,
                mask.as_ptr(),
                bgra.as_ptr(),
            )
        };
        if new_icon.is_null() {
            return Err(std::io::Error::last_os_error());
        }
        // The window borrows this handle. Retain it until it is replaced or detached.
        unsafe {
            SendMessageW(window, WM_SETICON, ICON_BIG as usize, new_icon as isize);
        }
        let old = std::mem::replace(&mut self.icon, new_icon);
        self.window = window;
        if !old.is_null() {
            unsafe {
                DestroyIcon(old);
            }
        }
        Ok(())
    }
}

impl Drop for TaskbarIcon {
    fn drop(&mut self) {
        if self.icon.is_null() {
            return;
        }
        // Do not clear an icon replaced by another owner, or touch a destroyed window.
        unsafe {
            if IsWindow(self.window) != 0
                && SendMessageW(self.window, WM_GETICON, ICON_BIG as usize, 0) == self.icon as isize
            {
                SendMessageW(self.window, WM_SETICON, ICON_BIG as usize, 0);
            }
            DestroyIcon(self.icon);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::ThemePalette;
    use cs2_resedit_core::UiTheme;
    use windows_sys::Win32::UI::WindowsAndMessaging::{CreateWindowExW, DestroyWindow, ICON_SMALL};

    #[test]
    fn native_taskbar_slot_updates_for_every_theme_and_detaches_on_drop() {
        let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
        // A hidden built-in window exercises the actual Win32 icon slots.
        let window = unsafe {
            CreateWindowExW(
                0,
                class.as_ptr(),
                std::ptr::null(),
                0,
                0,
                0,
                100,
                100,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null(),
            )
        };
        assert!(!window.is_null());
        struct Window(HWND);
        impl Drop for Window {
            fn drop(&mut self) {
                unsafe {
                    DestroyWindow(self.0);
                }
            }
        }
        let _window = Window(window);
        let mut taskbar = TaskbarIcon::default();
        for theme in UiTheme::ALL {
            let icon = ThemePalette::for_theme(theme).window_icon().unwrap();
            taskbar.set(window, &icon).unwrap();
            assert_eq!(
                unsafe { SendMessageW(window, WM_GETICON, ICON_BIG as usize, 0) },
                taskbar.icon as isize
            );
            assert_eq!(
                unsafe { SendMessageW(window, WM_GETICON, ICON_SMALL as usize, 0) },
                0
            );
        }
        drop(taskbar);
        assert_eq!(
            unsafe { SendMessageW(window, WM_GETICON, ICON_BIG as usize, 0) },
            0
        );
    }
}
