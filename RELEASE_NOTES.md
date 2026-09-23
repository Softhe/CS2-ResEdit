# CS2 ResEdit v1.1.0

CS2 ResEdit is a native, self-contained Windows editor for Counter-Strike 2 display settings. It discovers local Steam accounts, filters a curated resolution catalog by aspect family, reports target-display availability, and safely applies changes without requiring PowerShell or a separately installed .NET runtime.

## Highlights

- Hardened configuration parsing: capped VDF nesting and token counts, preserved escape sequences, and friendly errors for oversized or non-UTF-8 files.
- Safer apply flow: tri-state Counter-Strike 2 detection with confirmation when the game status is unknown, plus an explicit Reload/Overwrite/Cancel choice when the file changed on disk.
- Friendlier backups: readable names with timestamps and resolutions, newest-first selection, refresh action, and cleanup that can never undo a successful apply.
- More resilient discovery: cancelable Steam account refreshes, validated custom Steam locations with warnings, display-detection fallbacks, and digit-only custom resolution entry.
- Single-instance guard and `--help`/`--version` handling.

## Download

Download `CS2-ResEdit.exe` and `CS2-ResEdit.exe.sha256` from this release. The executable targets Windows 10/11 x64 and is self-contained.

The executable is unsigned, so Windows SmartScreen may warn on first launch. Verify the SHA-256 sidecar before running it.
