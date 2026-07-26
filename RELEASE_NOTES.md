# CS2 Video Config Editor v1.2.0

Version 1.2.0 makes configuration changes easier to review and recover.

## Highlights

- Responsive Windows 11-style dark interface with current and pending settings.
- Backup history with preview, guarded restore, and automatic rollback backups.
- Refreshable Steam accounts and locally remembered recent configuration files.
- More reliable PersonaName discovery for standard Steam `loginusers.vdf` files.
- Correct keyboard navigation through custom dimensions and Reset.
- Explicit Steam-root preservation when accounts are refreshed.
- Nonfatal inline warnings when old backups cannot be pruned.
- Automated Windows PowerShell 5.1 and PowerShell 7 tests.
- Deterministic ZIP packaging with a SHA-256 checksum.

The runtime remains a single dependency-free PowerShell script. Existing command-line parameters are unchanged.
