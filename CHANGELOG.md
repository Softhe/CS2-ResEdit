# Changelog

## Unreleased

- Apply and Restore ask for confirmation when `cs2.exe` is running, restoring the 1.x warning that the game may overwrite the file when it exits.
- Switching accounts, reloading, or opening another file while edits are pending asks before discarding them. Re-selecting the active account no longer reloads it.
- The palette follows Windows High Contrast at runtime instead of only the `CS2_RESEDIT_HIGH_CONTRAST` variable, so diagnostics report the real state.
- A failed reload after Restore is reported instead of being hidden behind "Backup restored successfully.", and a failed re-read after Apply says the change was written.
- Refresh is enabled again once a stalled account scan times out, matching `F5`. `Ctrl+S` requires exactly Ctrl.
- Display and High Contrast polling runs only while the window has focus. Portrait display modes are no longer listed under 4:3.
- Removed the redundant scan generation counters; only the newest scan is ever polled.
- Apply and Restore now work on read-only configurations and keep them read-only afterwards. Previously Windows rejected the replacement with "Access is denied".
- Failed Apply and Restore attempts no longer leave redundant backups that could push older backups out of retention.
- Configurations without `setting.aspectratiomode` now load and update. The aspect family is inferred from the dimensions and the field is never added.
- Duplicate resolution entries are rejected even when the extra entry has a malformed value, and a single malformed value now reports an invalid value instead of a missing entry.
- Backup path recognition uses the same Unicode case-insensitive comparison as the rest of the app, Steam account order is stable between refreshes, and `CS2_RESEDIT_SETTINGS_DIR` now takes effect on Windows.

- Restored diagnostics schema v2 with runtime description, high-contrast state, and recognized backup count. The report stays privacy-safe: counts and booleans only.
- Kept the preferences-load warning visible across the first configuration load instead of discarding it on save.
- Made footer operation results sticky: background scans and keystrokes no longer overwrite apply/restore failures, and account refresh confirmations survive auto-selection.
- Fixed tiny UTF-8 files being misdecoded as UTF-16, ASCII-only backup-name matching, and `CoreError::source()` for wrapped I/O errors.
- Removed the extra duplicate backup left behind after prune-failure rollbacks, and clean up stale atomic-write temporary files during backup retention.
- Replaced parallel pending-state fields with an explicit `EditState` (`NoFile`, `Invalid`, `Unchanged`, `Changed`).
- Extracted file navigation and editing transitions from rendering into `crates/app/src/app/controller.rs`.
- Generation-tagged background scans: timed-out scans can be replaced instead of wedging refreshes, and stale results are dropped.
- Added custom-dimension steppers, per-field errors with ranges, an always-visible Apply blocker reason, a stable preview while typing, and a no-backup warning.
- Improved the backups dialog (date-first rows, current-file comparison), made diagnostics read-only with a schema line, added guided empty states, a file sync badge in the header, and `F5`/`Ctrl+S`/`Esc` keyboard shortcuts.
- Added a `cargo-deny` advisory policy with a recorded tool version and exception policy, plus a weekly CI check. Fixed the intra-workspace wildcard dependency.
- Extended backup retention of CI release-candidate artifacts from 7 to 90 days.

## 2.0.0 - 2026-09-30

- Rewrote the C# and WinForms application in Rust with an egui interface.
- Replaced the .NET solution and build process with a Cargo workspace.
- Added four saved themes: Glacier, Paper, Monolith, and Canopy. Glacier is the default; Paper uses white running-window icons.
- Made title-bar and taskbar icons follow the theme.
- Added bounded wheel selection to all dropdowns and wheel adjustment to custom dimensions.
- Added Windows-reported monitor models and display IDs, with a graphics-adapter fallback.
- Reduced unused window space, aligned card heights, bounded long labels, and made startup sizing respect the work area and DPI.
- Fixed dropdown clicks that toggled menus in both the measurement and visible passes.
- Added background discovery, scan timeout notices, degraded-discovery warnings, and named accessible controls.
- Preserved the loaded configuration and pending edits when navigation fails or discovery refreshes the same file.
- Added last-configuration restoration and backward-compatible theme preferences.
- Rejected malformed Unicode and deeply nested KeyValues input, reserved recovery files without overwriting collisions, and guarded rollback against concurrent edits.
- Expanded regression tests for file preservation, recovery, state transitions, layout, pointer input, and native icons.
- Staged executable and checksum publication together, pinned the Rust toolchain, and updated CI.
- Rewrote the documentation for the Rust release.

## 1.1.0 - 2026-09-23

- Hardened configuration parsing and handling of files changed on disk.
- Improved backup naming, Steam discovery, display detection, and custom resolution entry.
- Added a single-instance guard and informational `--help` and `--version` commands.

## 1.0.0 - 2026-08-04

- Released the C# Windows application.
- Added local Steam discovery, configuration selection, display guidance, presets, and custom dimensions.
- Added a shared 16:9 preview canvas.
- Added atomic updates, encoding and line-ending preservation, backups, and rollback-safe restoration.
- Added local diagnostics that omit identifiers, paths, and configuration contents.
- Added the graphite and orange interface, DPI support, keyboard navigation, and accessible control names.
