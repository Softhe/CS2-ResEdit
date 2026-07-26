# Project roadmap

## v1.2.0

Version 1.2.0 completes the reliability and recovery milestone:

- Import-safe, testable single-script architecture.
- Windows PowerShell 5.1 and PowerShell 7 Pester coverage.
- Current-versus-pending preview, responsive layout, and Reset.
- Backup history, guarded atomic restore, rollback creation, and five-backup retention.
- Refreshable Steam accounts plus local last-account and recent-file preferences.
- Deterministic ZIP/checksum generation and tag-triggered release automation.

## v2.0.0

Version 2 completes the maintainability, diagnostics, and release-integrity milestone:

- Split video configuration, Steam discovery, preferences, and diagnostics into focused `.psm1` modules behind the compatible launcher.
- Add a privacy-safe diagnostic export with path and Steam-identifier anonymization.
- Add repeatable WinForms layout, DPI, High Contrast, keyboard, and accessibility regression tests.
- Replace runtime-dependent ZIP creation with a canonical cross-PowerShell builder and manifest verification.
- Add optional Authenticode signing tools and signature enforcement hooks for CI releases.
- Verify extracted release packages in a fresh PowerShell process.

## Future candidates

- Surface diagnostic export directly in the graphical interface.
- Localization-ready UI strings.
- Additional display-mode settings after Valve's config format is verified.
- Separate the large WinForms editor closure after its state and event contracts are stable.
