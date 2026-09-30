# Rust code audit, 2026-09-29

This report records findings from the September 29 audit. It is not a claim that every later release has been visually tested. The September 30 UI work and 2.0.0 release are recorded in the [changelog](CHANGELOG.md).

## Assessment

The application separates a native UI crate from a reusable core. Strict decoding, duplicate-key rejection, snapshot checks, and same-directory replacement provide a sound basis for configuration editing. Tests cover parsing, format preservation, backup naming, presets, and state changes.

The defects found in this pass were mainly at boundaries: calculated versus rendered layout, rollback versus external edits, partial Steam discovery, and executable/checksum publication. No evidence justified removing the Steam registry discovery, configuration parser, or resolution catalog.

## Findings and fixes

| Area | Finding | Change and evidence |
| --- | --- | --- |
| Layout | Frames could grow beyond their allocated rectangles. Long labels and a row containing the account selector and three buttons clipped the preview. | Put the account selector on its own bounded row, truncate long labels, wrap preview text, and size the preview from available height. The rendered-card test checks actual rectangles. |
| Startup | A fixed logical size could exceed a 1280x960 desktop at higher DPI. | Derive the non-resizable window size from the work area and DPI. The startup test covers 100%, 125%, and 150% scaling. |
| Selection | A failed load could leave the account selector pointing at a different file from the loaded state. Refresh could lose a custom entry. | Revert failed selection and retain the active custom path. Failed-load and refresh tests cover these transitions. |
| Rollback | Unconditional rollback after a replacement error could overwrite a concurrent edit. | Do not restore after a failed replacement. After a pruning failure, restore only while the live bytes match the app's write. The rollback test preserves a concurrent edit. |
| Restore | The backup was read once for validation and again for writing. | Write the exact validated buffer. Restore and pruning-failure tests exercise recovery. |
| Steam | Damaged login-user data silently removed persona names and ordering. | Continue discovery and report a warning. The degraded-discovery test covers malformed data. |
| Preferences | A saved path was accepted merely because it existed. | Validate it as a CS2 configuration before restoring selection. |
| Publishing | The default output path was broken, unsupported targets were accepted, and the executable was replaced before its checksum was ready. | Resolve paths after parameter binding, limit publication to Windows x64, stage both files, check the destination lock, and keep rollback copies during promotion. |
| CI | Toolchain selection and release-note checks did not agree with release requirements. | Pin the compiler and require notes to match the tag. The 2.0.0 preparation also replaced an invalid Rust action reference. |

The equal-height measurement pass later caused a dropdown regression because it shared IDs and processed clicks. The September 30 fix gave measurement widgets separate IDs and made the pass invisible and non-interactive. Pointer tests now check menu opening and selection.

## Remaining verification

Add Windows screenshot or accessibility checks at the supported desktop size and DPI scales. Geometry tests do not prove text rasterization, native chrome, or taskbar appearance. Test publication while the destination executable is running, and require successful CI before publishing a tag.

## Audit environment

The Windows workspace's Git pointer referred to an unavailable Linux worktree. The audit inspected source files directly rather than relying on Git diffs. The 2.0.0 release uses a fresh checkout and preserves the existing repository history.
