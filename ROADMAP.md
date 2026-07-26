# Project roadmap

## v1.2.0

Version 1.2.0 completes the reliability and recovery milestone:

- Import-safe, testable single-script architecture.
- Windows PowerShell 5.1 and PowerShell 7 Pester coverage.
- Current-versus-pending preview, responsive layout, and Reset.
- Backup history, guarded atomic restore, rollback creation, and five-backup retention.
- Refreshable Steam accounts plus local last-account and recent-file preferences.
- Deterministic ZIP/checksum generation and tag-triggered release automation.

## Future v2 work

Version 2 may split account discovery, config handling, preferences, and WinForms construction into `.psm1` modules. That larger packaging change should happen only when it provides a clear maintenance benefit over the v1.2 single-script distribution.

Additional candidates:

- Optional Authenticode signing and documented certificate handling.
- A dedicated diagnostic export with automatic path anonymization.
- Localization-ready UI strings.
- Additional display-mode settings after Valve's config format is verified.
