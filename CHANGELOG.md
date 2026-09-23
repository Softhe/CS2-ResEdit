# Changelog

All notable changes to CS2 ResEdit are documented here.

## 1.1.0 - 2026-09-23

- Hardened the VDF parser with nesting, token-count, and length caps; unknown escapes are preserved and unsupported directives are rejected.
- Replaced the fail-open game-running check with a tri-state probe that warns and confirms before applying when the status is unknown.
- Made preset population resilient to odd display-driver modes and hardened custom resolution input (length limit, digit-only entry, accessible descriptions).
- Added validation for custom Steam locations and preference paths with status warnings instead of silent empty results; corrupt-preference warnings no longer leak paths.
- Made Steam account discovery cancelable, added a single-instance guard, and added `--help`/`--version` handling.
- Reworked the backup dialog with friendly names, newest-first preselection, and a refresh action.
- Replaced the disk-conflict Yes/No prompt with an explicit Reload/Overwrite/Cancel choice, size-plus-timestamp change detection, and an unreadable-file warning.
- Unified resolution bounds handling and aligned aspect-tolerance checks; out-of-range loaded settings show a repair hint instead of blocking.
- Hardsized configuration reads with streaming size caps, friendlier encoding errors, and retention cleanup that can no longer roll back a successful apply.
- Improved display detection fallbacks, richer diagnostics error categories, and keyboard-shortcut tooltips.

## 1.0.0 - 2026-08-04

- Initial public release of the self-contained Windows application.
- Added local Steam account and CS2 configuration discovery.
- Added target-display awareness, curated presets, aspect-family defaults, and validated custom dimensions.
- Added a fixed 16:9 preview canvas that makes narrower aspect ratios directly comparable.
- Added atomic configuration updates with encoding and line-ending preservation.
- Added timestamped backups, rollback-safe restoration, and privacy-safe diagnostics.
- Added a warm-graphite dark interface with orange accents, per-monitor DPI support, keyboard navigation, and accessible control names.
