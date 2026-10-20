# CS2 ResEdit v2.1.0

Version 2.1.0 fixes problems found after the 2.0.0 Rust release and adds a fallback renderer. It edits the same CS2 configuration files, keeps the same preference location, and runs as a portable Windows x64 executable.

## Fixes

- Read-only `cs2_video.txt` files, a common way to stop CS2 from resetting them, can be applied and restored again. They stay read-only afterwards. Version 2.0.0 failed with "Access is denied".
- Configurations without `setting.aspectratiomode` load and update. The aspect family is inferred from the dimensions, and the field is never added.
- The editor no longer silently fails to appear. If it cannot open its window, an error dialog explains what failed.
- Failed Apply and Restore attempts no longer leave redundant backups that push older backups out of retention.
- Status messages no longer hide apply and restore failures behind background scan results or keystrokes.

## Changes

- When OpenGL cannot open the window, the editor switches to Direct3D 12. This helps on some virtual machines, Remote Desktop sessions, and older drivers.
- Apply and Restore ask for confirmation when Counter-Strike 2 is running, because the game can overwrite the file when it exits.
- Switching accounts, reloading, or opening another file asks before discarding unapplied edits.
- Custom width and height have `-` and `+` steppers and per-field errors. Apply shows why it is disabled.
- `F5` refreshes accounts, `Ctrl+S` applies, and `Esc` closes the topmost dialog.
- The palette follows Windows High Contrast while the editor runs.
- The account selector ignores the mouse wheel; displays, presets, aspect ratios, and themes still respond to it.
- Preferences are saved shortly after a change and when the window closes, rather than on every selection.

See the [changelog](CHANGELOG.md) for the complete list.

## Download and verification

Download `CS2-ResEdit.exe` and `CS2-ResEdit.exe.sha256`. This release supports Windows 10 and Windows 11, x64, with a graphics driver that supports OpenGL 2.0 or later, or Direct3D 12. No installer is required.

The executable is unsigned, so SmartScreen may warn. Compare its SHA-256 hash with the sidecar before running it. A matching checksum verifies file contents, not publisher identity.

Close CS2 before applying or restoring a configuration.
