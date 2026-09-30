# Changelog

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
