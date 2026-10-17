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
    // Players often mark cs2_video.txt read-only so the game cannot reset
    // it. Windows refuses to replace a read-only file, so lift the flag for
    // the swap and put it back afterwards to keep the user's lock in place.
    let read_only = match ReadOnlyGuard::lift(path) {
        Ok(guard) => guard,
        Err(err) => {
            let _ = fs::remove_file(&temporary);
            return Err(err);
        }
    };
    let replaced = replace_or_move(&temporary, path);
    // Best effort: the content swap is what callers act on, and the flag was
    // cleared moments ago, so restoring it is not expected to fail.
    read_only.restore(path);
    match replaced {
        Ok(()) => {
            // Post-condition: a successful swap leaves the target in place.
            if !path.is_file() {
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

/// Remembers whether a replacement target was read-only. On Windows the flag
/// is cleared so `ReplaceFileW` can swap the file; on every platform it is
/// reapplied to the new file afterwards.
struct ReadOnlyGuard(bool);

impl ReadOnlyGuard {
    fn lift(path: &Path) -> io::Result<Self> {
        let read_only = fs::metadata(path).is_ok_and(|m| m.permissions().readonly());
        #[cfg(windows)]
        if read_only {
            set_read_only(path, false)?;
        }
        Ok(Self(read_only))
    }

    fn restore(self, path: &Path) {
        if self.0 {
            let _ = set_read_only(path, true);
        }
    }
}

fn set_read_only(path: &Path, read_only: bool) -> io::Result<()> {
    let mut permissions = fs::metadata(path)?.permissions();
    // Only reached with `false` on Windows, where it clears the read-only
    // attribute rather than granting write access to everyone.
    #[allow(clippy::permissions_set_readonly_false)]
    permissions.set_readonly(read_only);
    fs::set_permissions(path, permissions)
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

/// Case-insensitive path comparison; Windows paths use Unicode case folding.
pub fn paths_equal_ci(a: &str, b: &str) -> bool {
    #[cfg(windows)]
    {
        a.to_lowercase() == b.to_lowercase()
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

/// Best-effort removal of stale `atomic_write` temporary files
/// (`.{name}.{pid}.{nanos}.{attempt}.tmp`) left behind by crashes.
/// Only files older than `max_age_secs` are removed; errors are ignored.
pub fn sweep_stale_temp_files(target: &Path, max_age_secs: u64) {
    let (Some(directory), Some(target_name)) = (target.parent(), target.file_name()) else {
        return;
    };
    let prefix = format!(".{}.", target_name.to_string_lossy());
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    let now = SystemTime::now();
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        let Some(suffix) = name
            .strip_prefix(&prefix)
            .and_then(|s| s.strip_suffix(".tmp"))
        else {
            continue;
        };
        let parts: Vec<_> = suffix.split('.').collect();
        if parts.len() != 3
            || parts
                .iter()
                .any(|part| part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()))
            || parts[0].parse::<u32>().is_err()
            || parts[1].parse::<u128>().is_err()
            || parts[2].parse::<u32>().is_err()
        {
            continue;
        }
        let stale = fs::symlink_metadata(&path)
            .and_then(|m| {
                if m.is_file() {
                    m.modified()
                } else {
                    Err(io::Error::other("not a regular file"))
                }
            })
            .ok()
            .and_then(|t| now.duration_since(t).ok())
            .is_some_and(|age| age.as_secs() >= max_age_secs);
        if stale {
            let _ = fs::remove_file(&path);
        }
    }
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
    use windows_sys::Win32::Storage::FileSystem::{ReplaceFileW, REPLACEFILE_IGNORE_MERGE_ERRORS};

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
    // Both names are NUL-terminated; no backup file, exclude, or reserved data.
    let ok = unsafe {
        ReplaceFileW(
            replaced_wide.as_ptr(),
            replacement_wide.as_ptr(),
            std::ptr::null(),
            REPLACEFILE_IGNORE_MERGE_ERRORS,
            std::ptr::null(),
            std::ptr::null(),
        )
    };
    if ok != 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleanup_only_removes_owned_target_temps() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("cs2_video.txt");
        let owned = dir.path().join(".cs2_video.txt.123.456.0.tmp");
        let unrelated = [
            ".other-tool.tmp",
            ".other.txt.123.456.0.tmp",
            ".cs2_video.txt.bad.456.0.tmp",
            ".cs2_video.txt.123.456.tmp",
            ".cs2_video.txt.123.456.0.extra.tmp",
            ".cs2_video.txt.123..0.tmp",
        ];
        fs::write(&owned, b"owned").unwrap();
        for name in unrelated {
            fs::write(dir.path().join(name), b"keep").unwrap();
        }
        let folder = dir.path().join(".cs2_video.txt.123.456.1.tmp");
        fs::create_dir(&folder).unwrap();
        sweep_stale_temp_files(&target, u64::MAX);
        assert!(owned.exists(), "fresh owned temp must survive");
        sweep_stale_temp_files(&target, 0);
        assert!(!owned.exists());
        for name in unrelated {
            assert!(dir.path().join(name).exists(), "deleted {name}");
        }
        assert!(folder.is_dir());
    }

    #[cfg(windows)]
    #[test]
    fn unicode_windows_paths_share_comparison_and_key() {
        let upper = r"C:\Équipe\cs2_video.txt";
        let lower = r"C:\équipe\cs2_video.txt";
        assert!(paths_equal_ci(upper, lower));
        assert_eq!(path_key(Path::new(upper)), path_key(Path::new(lower)));
    }
}
