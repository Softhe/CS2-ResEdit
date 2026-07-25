# Roadmap to v2.0

The second release should focus on maintainability, recoverability, and a more dependable distribution process. These five steps are ordered so each one reduces risk for the next.

## 1. Split the application into testable components

Move account discovery, VDF/config parsing, configuration updates, and UI construction into focused `.psm1` modules while keeping `CS2-VideoConfig-Editor.ps1` as the stable entry point.

**Done when:** the existing GUI and command-line parameters behave unchanged, module boundaries are documented, and core functions can be imported without launching the interface.

## 2. Add automated tests and continuous integration

Create Pester coverage for SteamID conversion, account discovery, malformed and duplicate VDF entries, preset/custom resolutions, encoding and CRLF preservation, atomic replacement, backup creation, and no-change updates. Run the suite on Windows PowerShell 5.1 and PowerShell 7 in GitHub Actions.

**Done when:** pull requests receive repeatable parser and regression results on both supported PowerShell versions.

## 3. Add backup history, preview, and restore

Show the exact old and new resolution values before applying, list backups created by the editor, and provide a guarded restore action. Keep every write atomic and verify the restored file before reporting success.

**Done when:** a user can preview a change, apply it, and restore the previous configuration without manually renaming files.

## 4. Strengthen discovery and UI workflows

Add an account refresh action, clearer handling for accounts without a CS2 config, recent custom-file history, and improved status details for permission or file-lock failures. Complete keyboard, High Contrast, long-text, and 100-200% DPI checks.

**Done when:** first-run, multi-account, missing-config, custom-path, and failure-recovery flows are covered by a written UI test checklist.

## 5. Automate and harden the release

Create a release workflow that validates the source, builds the ZIP from the tagged commit, checks the archive hash, publishes checksums, and attaches all artifacts to a GitHub Release. Add release notes and document an optional code-signing path.

**Done when:** tagging `v2.0.0` produces reproducible, verified downloads and the README download link resolves to those artifacts.
