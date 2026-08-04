# CS2 ResEdit v1.0.0

CS2 ResEdit is a native, self-contained Windows editor for Counter-Strike 2 display settings. It discovers local Steam accounts, filters a curated resolution catalog by aspect family, reports target-display availability, and safely applies changes without requiring PowerShell or a separately installed .NET runtime.

## Highlights

- Warm-graphite dark interface with the `#F89D1C` orange accent and sharp per-monitor high-DPI rendering.
- Target-display discovery and Windows-reported mode guidance across 43 curated presets.
- Recommended defaults of `1280x960` for 4:3, `1920x1080` for 16:9, and `1680x1050` for 16:10.
- Custom width and height fields that appear only in Custom resolution mode.
- Fixed 16:9 preview canvas so 4:3, 5:4, and 16:10 pillarboxing is immediately visible.
- Atomic VDF updates that preserve UTF-8/UTF-16 encoding, BOM state, line endings, and unrelated values.
- Timestamped backups, five-backup retention, previewed restoration, and rollback backup creation.
- Privacy-safe diagnostics that exclude account identifiers, paths, preferences, and configuration contents.

## Download

Download `CS2-ResEdit.exe` and `CS2-ResEdit.exe.sha256` from this release. The executable targets Windows 10/11 x64 and is self-contained.

The executable is unsigned, so Windows SmartScreen may warn on first launch. Verify the SHA-256 sidecar before running it.
