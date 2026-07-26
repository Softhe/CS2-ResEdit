# v1.2.0 release checklist

## Local release candidate

- [x] Application version and release-note heading are both `v1.2.0`.
- [x] Windows PowerShell 5.1 parser and Pester suite pass.
- [x] PowerShell 7 parser and Pester suite pass.
- [x] Main source is ASCII-compatible.
- [x] GUI account, recent-file, keyboard, Reset, Apply, and validation workflows are verified.
- [x] Backup restore, rollback creation, retention, and failure handling are verified.
- [x] Preferences remain isolated from non-GUI modes.
- [x] GitHub Actions workflows pass actionlint.
- [x] CI actions use immutable commit SHAs.
- [x] Two clean release builds produce an identical ZIP hash.
- [x] ZIP contains only `CS2-VideoConfig-Editor.ps1`.
- [x] Packaged source hash matches the repository source.
- [x] SHA-256 sidecar matches the final ZIP.
- [x] `git diff --check` passes.

## Maintainer approval and publication

- [x] Review the complete uncommitted diff.
- [x] Approve the v1.2.0 release notes and screenshot.
- [x] Commit the intended release-candidate files.
- [x] Push the commit and confirm the Test workflow succeeds.
- [x] Create and push the signed or annotated `v1.2.0` tag.
- [x] Confirm the Release workflow publishes the ZIP and checksum.
- [x] Download the published assets and compare their hashes with the workflow output.
