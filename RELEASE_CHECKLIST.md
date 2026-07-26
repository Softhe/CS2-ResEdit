# v2.0.0 release checklist

## Local release candidate

- [x] Application version and release-note heading are both `v2.0.0`.
- [x] Windows PowerShell 5.1 parser and Pester suite pass.
- [x] PowerShell 7 parser and Pester suite pass.
- [x] Main source and runtime modules are ASCII-compatible.
- [x] GUI account, recent-file, keyboard, Reset, Apply, and validation workflows are verified.
- [x] Backup restore, rollback creation, retention, and failure handling are verified.
- [x] Preferences remain isolated from non-GUI modes.
- [x] Diagnostics export is anonymized and contains no raw account, persona, or filesystem identifiers.
- [x] GitHub Actions workflows pass actionlint.
- [x] CI actions use immutable commit SHAs.
- [x] Two clean release builds produce an identical ZIP hash.
- [x] Windows PowerShell 5.1 and PowerShell 7 produce byte-identical ZIPs.
- [x] ZIP contains exactly the runtime files listed in `build/ReleaseFiles.psd1`.
- [x] Every packaged runtime file is byte-identical to its repository source.
- [x] SHA-256 sidecar exactly matches the final ZIP.
- [x] Extracted-package launcher smoke test passes.
- [x] Isolated packaged-app preset and diagnostics flow passes in Windows PowerShell 5.1 and PowerShell 7.
- [x] `git diff --check` passes.

## Maintainer approval and publication

- [ ] Review the complete uncommitted diff.
- [ ] Approve the v2.0.0 release notes and screenshot.
- [ ] Commit the intended release-candidate files.
- [ ] Push the commit and confirm the Test workflow succeeds.
- [ ] Create and push the signed or annotated `v2.0.0` tag.
- [ ] Confirm the Release workflow publishes the ZIP and checksum.
- [ ] Download the published assets and compare their hashes with the workflow output.

## Optional Authenticode enforcement

- [ ] Sign every PowerShell runtime file in `build/ReleaseFiles.psd1` with a trusted code-signing certificate.
- [ ] Verify the signatures locally with `build/Test-ReleaseArtifacts.ps1 -RequireAuthenticodeSignature`.
- [ ] Set the repository variable `REQUIRE_AUTHENTICODE_SIGNATURE` to `true`.
- [ ] Confirm both Test and Release workflows reject unsigned or invalid runtime files.
