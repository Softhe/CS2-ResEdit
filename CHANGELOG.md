# Changelog

All notable changes to CS2 Video Config Editor are documented here.

## 2.0.1 - 2026-07-27

- Added `1728x1080` to the built-in 16:10 resolution presets.
- Fixed packaged end-to-end fixture normalization for repositories checked out with CRLF line endings.

## 2.0.0 - 2026-07-26

- Split configuration handling, Steam discovery, preferences, and diagnostics into focused PowerShell modules.
- Added `-ExportDiagnostics` for atomic, privacy-safe JSON support reports without configuration or preference changes.
- Added WinForms regression coverage for responsive layouts, DPI scaling, High Contrast, keyboard navigation, accessibility labels, and safe action states.
- Replaced runtime-dependent ZIP output with a canonical manifest-driven builder that is byte-identical in Windows PowerShell 5.1 and PowerShell 7.
- Added exact package-entry verification and a fresh-process smoke test of the extracted release.
- Added optional Authenticode signing tools and release signature enforcement.
- Fixed rounded card repainting so repeated window resizing no longer leaves stale horizontal border artifacts.

## 1.2.0 - 2026-07-26

- Added a responsive current-versus-pending display workflow with Reset and no-change detection.
- Added local Steam account refresh, remembered account selection, and up to five recent custom configuration files.
- Added backup history with guarded, atomic restore and automatic retention of the newest five editor backups.
- Fixed PersonaName discovery from CRLF-formatted `loginusers.vdf` files.
- Added Pester coverage for parsing, updates, backups, restore, retention, preferences, and UI-independent state.
- Added Windows PowerShell 5.1 and PowerShell 7 CI plus deterministic ZIP and checksum generation.
- Fixed account refresh so an explicitly supplied `-SteamRoot` remains in effect.
- Fixed nested WinForms tab ordering so Custom width, Custom height, and Reset are keyboard reachable.
- Made backup retention failures nonfatal after a successful Apply or Restore and exposed the warning inline.
- Pinned CI actions to immutable commits and added reproducible artifact verification.

## 1.1.0 - 2026-07-24

- Refreshed the dark Windows interface and added local Steam PersonaName discovery.
- Fixed configuration validation and backup handling.
- Removed the obsolete updater and published verified ZIP/checksum assets.

## 1.0.0

- Initial release.
