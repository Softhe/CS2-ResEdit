//! Shared atomic file replacement and path helpers.
//!
//! Mirrors the .NET `File.Replace` / `File.Move` behavior used by the C#
//! implementation: the replacement is performed atomically by the OS where
//! supported, reducing the chance of a missing or truncated target after a crash.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Writes `bytes` to a sibling temporary file and atomically moves it onto
/// `path`. On Windows an existing target is replaced via `ReplaceFileW`.
/// The temporary file is flushed before the swap. Durability still depends on
/// filesystem and platform behavior.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let directory = path.parent().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "path has no parent directory")
    })?;
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "file".to_string());
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let (temporary, mut file) = (0_u32..)
        .find_map(|attempt| {
            let candidate = directory.join(format!(
                ".{}.{}.{}.{}.tmp",
                file_name,
                std::process::id(),
                nanos,
                attempt
            ));
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&candidate)
            {
                Ok(file) => Some(Ok((candidate, file))),
                Err(err) if err.kind() == io::ErrorKind::AlreadyExists => None,
                Err(err) => Some(Err(err)),
            }
        })
        .expect("the temporary-file attempt counter is unbounded")?;
    if let Err(err) = file.write_all(bytes).and_then(|_| file.sync_all()) {
        drop(file);
        let _ = fs::remove_file(&temporary);
        return Err(err);
    }
    drop(file);
    match replace_or_move(&temporary, path) {
        Ok(()) => {
            // Post-condition guard: after a successful swap the target must
            // exist and the temporary must be consumed. If a replace call
            // ever misbehaves the way the original argument-order bug did
            // (target deleted while the temporary survives holding the old
            // content), restore the target instead of leaving the game
            // without its configuration.
            if !path.exists() {
                if temporary.exists() {
                    fs::copy(&temporary, path)?;
                    let _ = fs::remove_file(&temporary);
                    return Err(io::Error::other(
                        "the replacement did not take effect; the original file was restored",
                    ));
                }
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    "target file missing after atomic replace",
                ));
            }
            let _ = fs::remove_file(&temporary);
            // Flush the directory entry so the rename itself survives power
            // loss where supported. Windows durability depends on the filesystem.
            #[cfg(not(windows))]
            if let Ok(dir) = fs::File::open(directory) {
                let _ = dir.sync_all();
            }
            Ok(())
        }
        Err(err) => {
            let _ = fs::remove_file(&temporary);
            Err(err)
        }
    }
}

pub fn absolute(path: &Path) -> io::Result<PathBuf> {
    let resolved = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    Ok(to_native(&resolved))
}

pub fn absolute_lossy(path: &Path) -> PathBuf {
    to_native(&absolute(path).unwrap_or_else(|_| {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            PathBuf::from(".").join(path)
        }
    }))
}

/// Case-insensitive path comparison; Windows filesystems ignore ASCII case.
pub fn paths_equal_ci(a: &str, b: &str) -> bool {
    #[cfg(windows)]
    {
        a.eq_ignore_ascii_case(b)
    }
    #[cfg(not(windows))]
    {
        a == b
    }
}

/// Platform-aware key for deduplicating absolute paths.
pub fn path_key(path: &Path) -> String {
    let value = absolute_lossy(path).to_string_lossy().to_string();
    #[cfg(windows)]
    {
        value.to_lowercase()
    }
    #[cfg(not(windows))]
    {
        value
    }
}

/// Nanosecond-resolution mtime used to keep rapid successive backups in order.
pub fn file_mtime_nanos(path: &Path) -> i64 {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_nanos().min(i64::MAX as u128) as i64)
        .unwrap_or(0)
}

fn replace_or_move(temporary: &Path, path: &Path) -> io::Result<()> {
    if !path.exists() {
        return fs::rename(temporary, path);
    }
    #[cfg(windows)]
    {
        // Normalize separators: Steam's registry stores SteamPath with
        // forward slashes, and ReplaceFileW is stricter about them than
        // most Win32 file APIs.
        let replaced = to_native(path);
        let replacement = to_native(temporary);
        match replace_file(&replaced, &replacement) {
            Ok(()) => Ok(()),
            Err(err) => {
                if path.exists() {
                    Err(err)
                } else {
                    fs::rename(temporary, path)
                }
            }
        }
    }
    #[cfg(not(windows))]
    {
        fs::rename(temporary, path)
    }
}

/// Converts forward slashes to the native separator on Windows so paths
/// coming from Steam's registry behave like the C# build's GetFullPath
/// output; a no-op elsewhere.
pub fn to_native(path: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::{OsStrExt, OsStringExt};
        let wide: Vec<u16> = path
            .as_os_str()
            .encode_wide()
            .map(|unit| {
                if unit == '/' as u16 {
                    '\\' as u16
                } else {
                    unit
                }
            })
            .collect();
        PathBuf::from(std::ffi::OsString::from_wide(&wide))
    }
    #[cfg(not(windows))]
    {
        path.to_path_buf()
    }
}

#[cfg(windows)]
fn replace_file(replaced: &Path, replacement: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;

    const REPLACEFILE_IGNORE_MERGE_ERRORS: u32 = 0x2;

    #[link(name = "kernel32")]
    extern "system" {
        fn ReplaceFileW(
            lp_replaced_file_name: *const u16,
            lp_replacement_file_name: *const u16,
            lp_backup_file_name: *const u16,
            dw_replace_flags: u32,
            lp_exclude: *mut core::ffi::c_void,
            lp_reserved: *mut core::ffi::c_void,
        ) -> i32;
    }

    // Argument order matters and is easy to get wrong:
    // lpReplacedFileName = the LIVE file being replaced (old content),
    // lpReplacementFileName = the new file that takes its place. Passing
    // these backwards deletes the live target instead of updating it.
    fn to_wide(path: &Path) -> Vec<u16> {
        path.as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    let replaced_wide = to_wide(replaced);
    let replacement_wide = to_wide(replacement);
    let ok = unsafe {
        ReplaceFileW(
            replaced_wide.as_ptr(),
            replacement_wide.as_ptr(),
            std::ptr::null(),
            REPLACEFILE_IGNORE_MERGE_ERRORS,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if ok != 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}
