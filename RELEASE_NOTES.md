# CS2 ResEdit v2.0.0

Version 2.0.0 replaces the C# and WinForms application with a Rust application built with egui. The major version marks that implementation change. It still edits the same CS2 configuration files and runs as a portable Windows x64 executable.

## Changes since 1.1.0

- Replaced the .NET source and build process with a Cargo workspace containing a reusable core and desktop UI.
- Added Glacier, Paper, Monolith, and Canopy themes. Glacier is the default, and the selected theme persists between launches.
- Linked running title-bar and taskbar icons to the theme. Paper uses white icons.
- Added bounded wheel selection to all closed dropdowns and wheel adjustment to custom dimensions.
- Added Windows-reported monitor models and display IDs, with an adapter-name fallback.
- Made the fixed-size window compact, aligned the cards, and bounded long labels so they do not push the preview off-screen.
- Moved Steam and display scans to background workers and added notices for slow or degraded discovery.
- Expanded regression tests for malformed input, backups, file-change guards, failed navigation, UI interaction, layout, and native icons.

## File handling

The editor preserves encoding, byte-order marks, line endings, and unrelated configuration values. Apply and Restore reject a file that changed after loading. Backups remain enabled by default, and restoration creates a rollback backup first.

The display selector provides availability guidance. It does not change the desktop resolution or CS2's selected monitor. Close CS2 before applying or restoring a configuration.

## Upgrade

Close version 1.1.0 and replace its executable with `CS2-ResEdit.exe` from this release. Existing CS2 files and recognized backups remain usable. No separately installed .NET runtime is needed.

Preferences remain in `%LOCALAPPDATA%\Softhe\CS2-ResEdit\v1\settings.json`. The directory name refers to the preference schema. Older settings without a theme default to Glacier. Settings in other product directories are not imported.

## Download and verification

Download `CS2-ResEdit.exe` and `CS2-ResEdit.exe.sha256`. This release supports Windows 10 and Windows 11, x64, with a graphics driver that supports OpenGL 2.0 or later. No installer is required.

The executable is unsigned, so SmartScreen may warn. Compare its SHA-256 hash with the sidecar before running it. A matching checksum verifies file contents, not publisher identity.

The executable's File Explorer icon remains static. Theme colors apply to the running title-bar and taskbar icons.
