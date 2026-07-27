<div align="center">

# CS2 Video Config Editor

Safely change Counter-Strike 2 resolution settings from a modern Windows interface.

[![Latest release](https://img.shields.io/github/v/release/Softhe/CS2-VideoConfig-Editor?display_name=tag&sort=semver&style=flat-square)](https://github.com/Softhe/CS2-VideoConfig-Editor/releases/latest)
[![PowerShell 5.1+](https://img.shields.io/badge/PowerShell-5.1%2B-5391FE?logo=powershell&logoColor=white&style=flat-square)](https://learn.microsoft.com/powershell/)
[![Windows 10/11](https://img.shields.io/badge/Windows-10%20%7C%2011-0078D4?logo=windows11&logoColor=white&style=flat-square)](#requirements)
[![Tests](https://img.shields.io/github/actions/workflow/status/Softhe/CS2-VideoConfig-Editor/test.yml?branch=main&label=tests&style=flat-square)](https://github.com/Softhe/CS2-VideoConfig-Editor/actions/workflows/test.yml)
[![No dependencies](https://img.shields.io/badge/dependencies-none-2EA44F?style=flat-square)](#requirements)

### [Download the latest ZIP](https://github.com/Softhe/CS2-VideoConfig-Editor/releases/latest/download/CS2-VideoConfig-Editor.zip)

[View releases](https://github.com/Softhe/CS2-VideoConfig-Editor/releases/latest) ·
[SHA-256 checksum](https://github.com/Softhe/CS2-VideoConfig-Editor/releases/latest/download/CS2-VideoConfig-Editor.zip.sha256) ·
[Roadmap](ROADMAP.md)

</div>

<p align="center">
  <img src="assets/cs2-video-config-editor.png" alt="CS2 Video Config Editor showing Steam account and display settings" width="780">
</p>

## Overview

CS2 Video Config Editor finds the video configuration for each local Steam account and updates the selected resolution without requiring manual VDF editing.

| Capability | What it does |
| --- | --- |
| **Multiple Steam accounts** | Shows each local PersonaName, Account ID, and SteamID64. |
| **Common and custom resolutions** | Includes 23 validated `4:3 / 5:4`, `16:9`, and `16:10` presets through 4K-class dimensions, plus exact custom dimensions. |
| **Safe file updates** | Validates required entries, preserves encoding and line endings, and replaces the file atomically. |
| **Automatic backups** | Creates a timestamped `.bak` copy before a change unless explicitly disabled. |
| **Backup restore** | Previews editor backups and restores one atomically while preserving the active file. |
| **Responsive workflow** | Compares current and pending settings, supports Reset, and reflows for narrow windows. |
| **Remembered files** | Remembers the last Steam account and up to five recent custom files locally. |
| **Flexible operation** | Supports the graphical editor, an interactive console, and automation-friendly parameters. |
| **Private diagnostics** | Exports a read-only JSON support report with paths and Steam identifiers anonymized. |
| **Local and private** | Reads Steam account names locally; no API key, login, or online account lookup is used. |

## Quick start

1. [Download the latest ZIP](https://github.com/Softhe/CS2-VideoConfig-Editor/releases/latest/download/CS2-VideoConfig-Editor.zip).
2. Extract the complete archive, keeping the `modules` folder beside `CS2-VideoConfig-Editor.ps1`.
3. Close Counter-Strike 2.
4. Right-click `CS2-VideoConfig-Editor.ps1` and select **Run with PowerShell**.
5. Choose a Steam account and resolution, then select **Apply changes**.

Use **Refresh** to rescan local Steam accounts. Use **Backups...** to preview or restore a configuration previously backed up by the editor.

You can also launch it from a PowerShell prompt:

```powershell
.\CS2-VideoConfig-Editor.ps1
```

If Windows blocked the downloaded file, right-click the ZIP before extracting it, open **Properties**, select **Unblock**, and apply the change. Alternatively:

```powershell
Get-ChildItem . -Recurse -File | Unblock-File
```

## How account discovery works

The editor searches Steam's local `userdata` folders and converts each 32-bit Account ID to its SteamID64. Persona names are matched from Steam's local `config\loginusers.vdf`.

The usual configuration location is:

```text
C:\Program Files (x86)\Steam\userdata\<AccountID>\730\local\cfg\cs2_video.txt
```

Use **Browse** or `-FilePath` when Steam or the configuration is stored elsewhere.

The graphical editor remembers the last selected Steam Account ID and up to five valid custom configuration paths in:

```text
%LOCALAPPDATA%\Softhe\CS2-VideoConfig-Editor\settings.json
```

This file contains no credentials and is never uploaded. Console, preset, silent, and account-listing modes do not read or update GUI preferences.

## Command-line usage

### Graphical editor

```powershell
.\CS2-VideoConfig-Editor.ps1
```

### Interactive console

```powershell
.\CS2-VideoConfig-Editor.ps1 -Console
```

### Apply a resolution directly

```powershell
.\CS2-VideoConfig-Editor.ps1 -Preset 1920x1080
```

For a custom configuration path or explicit aspect mode:

```powershell
.\CS2-VideoConfig-Editor.ps1 `
  -FilePath 'D:\Steam\userdata\123456\730\local\cfg\cs2_video.txt' `
  -Preset 1440x1080 `
  -AspectRatioMode 4:3
```

List discovered Steam accounts:

```powershell
.\CS2-VideoConfig-Editor.ps1 -ListAccounts
```

Export a privacy-safe diagnostic report without changing the configuration or
graphical-interface preferences:

```powershell
.\CS2-VideoConfig-Editor.ps1 `
  -ExportDiagnostics .\cs2-diagnostics.json
```

The report contains runtime and validation status, anonymized paths, aggregate
account discovery results, and at most 20 structured operation records. It does
not contain PersonaName, AccountName, Account ID, SteamID64, raw configuration
text, environment dumps, hashes, or stack traces.

### Parameters

| Parameter | Description |
| --- | --- |
| `-FilePath <path>` | Use a specific `cs2_video.txt`. |
| `-SteamRoot <path>` | Search a nonstandard Steam installation. |
| `-Preset <width>x<height>` | Apply a predefined or custom resolution. A one-based preset number is also accepted. |
| `-AspectRatioMode <value>` | Use `4:3`, `16:9`, `16:10`, or the corresponding modes `0`, `1`, `2`. |
| `-Console` | Open the interactive console instead of the graphical editor. |
| `-ListAccounts` | Print locally discovered accounts and configuration paths. |
| `-ExportDiagnostics <path>` | Write an anonymized JSON diagnostic report and exit without modifying application data. |
| `-NoBackup` | Apply the change without retaining a timestamped backup. |
| `-Silent` | Suppress informational output in preset, account-listing, or diagnostic-export modes. |

## Safety and recovery

- Close CS2 before applying a change. A running game may overwrite its configuration when it exits.
- Keep **Create a timestamped backup** enabled unless you have another recovery method.
- Backups are written beside the original file as `cs2_video.txt.<timestamp>.bak`.
- Select **Backups...** to preview and restore one. Restore creates another rollback backup first.
- The editor retains the newest five backups it created for each configuration and never prunes unrelated `.bak` files.

The editor validates all required settings in memory before writing. An invalid or incomplete configuration is rejected without modifying the file.

## Verify the download

Download both release assets into the same directory:

- `CS2-VideoConfig-Editor.zip`
- `CS2-VideoConfig-Editor.zip.sha256`

Then run:

```powershell
$expected = (Get-Content .\CS2-VideoConfig-Editor.zip.sha256).Split()[0]
$actual = (Get-FileHash .\CS2-VideoConfig-Editor.zip -Algorithm SHA256).Hash.ToLowerInvariant()
$actual -eq $expected
```

The result should be `True`.

## Requirements

- Windows 10 or Windows 11
- Windows PowerShell 5.1 or PowerShell 7+
- Counter-Strike 2 installed through Steam, or a valid `cs2_video.txt` supplied manually

No installation, administrator rights, Steam Web API key, or third-party PowerShell module is required.

## Development and testing

The v2 runtime uses a small launcher plus focused modules for video configuration,
Steam discovery, preferences, and diagnostics. Keep the packaged directory
structure intact. Tests use Pester 5.8.0 only during development and CI:

```powershell
Invoke-Pester .\tests
.\build\Invoke-GuiRegression.ps1
.\build\Invoke-PackagedAppE2E.ps1
```

Build the deterministic ZIP and checksum locally with:

```powershell
.\build\Build-Release.ps1 -Version 2.1.0
```

The canonical builder produces byte-identical archives in Windows PowerShell 5.1
and PowerShell 7, verifies every manifest entry against the repository, and runs
the extracted launcher. Optional Authenticode signing is documented in
[`build/AUTHENTICODE.md`](build/AUTHENTICODE.md).

Pushing a matching `v*` tag runs both supported PowerShell test environments before the release workflow publishes the verified assets. CI actions are pinned to immutable commits and compare independent Windows PowerShell and PowerShell 7 builds before uploading the candidate artifacts.

See the [release checklist](RELEASE_CHECKLIST.md) before creating a version tag.

## Roadmap

See the [project roadmap](ROADMAP.md) for completed milestones and future candidates.

## Releases

Release notes, packaged downloads, and checksums are available on the [GitHub Releases page](https://github.com/Softhe/CS2-VideoConfig-Editor/releases).
