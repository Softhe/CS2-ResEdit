# CS2 ResEdit

CS2 ResEdit edits Counter-Strike 2 resolution and aspect settings on Windows. Version 2.0.0 replaces the C# and WinForms application with a Rust application built with egui.

[Download CS2-ResEdit.exe](https://github.com/Softhe/CS2-ResEdit/releases/latest/download/CS2-ResEdit.exe) · [Release notes](https://github.com/Softhe/CS2-ResEdit/releases/latest) · [SHA-256 checksum](https://github.com/Softhe/CS2-ResEdit/releases/latest/download/CS2-ResEdit.exe.sha256)

## Requirements

- Windows 10 or Windows 11, x64.
- A graphics driver that supports OpenGL 2.0 or later.
- A Steam installation with a CS2 video configuration, or a manually selected `cs2_video.txt`.

The portable executable needs no installer, administrator rights, PowerShell runtime, or separately installed .NET runtime. The application works locally and does not upload your configuration or Steam account information.

## Use the editor

1. Close Counter-Strike 2. The game can overwrite its configuration when it exits.
2. Download and run `CS2-ResEdit.exe`.
3. Select a Steam account, or choose Browse to open a configuration.
4. Select the target display, aspect ratio, and resolution.
5. Review the current and pending values, then choose Apply changes.

The usual configuration path is:

```text
C:\Program Files (x86)\Steam\userdata\<AccountID>\730\local\cfg\cs2_video.txt
```

Steam and display discovery run in the background. Refresh scans accounts again. Reload file reads the selected configuration again and replaces pending values with its current settings. Reset discards pending edits without writing to the file.

Only one editor instance can run per Windows session. `--help` and `--version` show informational dialogs in the release build. Command-line configuration editing is not supported.

## Displays and resolutions

The display list shows the Windows-reported monitor model, display ID, current resolution, and primary-display status. For example, a monitor may appear as `ROG PG27AQN · DISPLAY1 · 2560x1440 · Primary`. If Windows cannot provide a monitor name, the editor falls back to the graphics-adapter name.

Selecting a display changes resolution-availability guidance. It does not switch the desktop resolution or change CS2's monitor-selection fields.

Windows-reported modes appear first, followed by the remaining curated presets for the selected aspect family. An unreported mode remains selectable; that label is not a guarantee that the monitor or game supports it.

Changing the aspect family selects its recommended preset:

| Aspect family | Recommended resolution |
| --- | --- |
| 4:3 / 5:4 | 1280x960 |
| 16:9 | 1920x1080 |
| 16:10 | 1680x1050 |

The catalog contains 43 presets. Choose Custom resolution to enter a width from 320 to 32768 and a height from 200 to 32768. The shared 16:9 preview canvas shows how narrower aspect ratios compare. It does not predict the game's scaling or stretching behavior.

## Themes and mouse wheel

Choose Theme in the header. The editor saves your choice for the next launch.

| Theme | UI colors | Running-window icons |
| --- | --- | --- |
| Glacier, the default | Dark blue and cyan | Cyan |
| Paper | Light cream and teal | White |
| Monolith | Dark graphite and lime | Lime |
| Canopy | Dark forest green and jade | Jade |

The title-bar and running taskbar icons follow the theme. The executable's File Explorer icon remains static.

Hover over any closed dropdown and scroll down to select the next entry or up to select the previous entry. Selection stops at either end. This applies to accounts, displays, presets, aspect ratios, and themes. Open menus retain normal scrolling.

The custom width and height fields also accept wheel input. Scroll up to increase a value or down to decrease it. Adjustments stay within the allowed dimension range.

## File changes and recovery

Apply validates the required fields, prepares the update in memory, and replaces the configuration through a temporary file in the same directory. It preserves UTF-8 or UTF-16 encoding, byte-order marks, line endings, and unrelated values. Files without the retired legacy aspect-mode field remain supported.

If another program changes the loaded file, Apply and Restore stop and report the difference. Choose Reload file, review the new values, then make your change again. These checks reduce the risk of overwriting external edits, but they cannot lock out every write from another process.

Create a timestamped backup before applying is enabled by default. Backups are stored beside the configuration with names such as `cs2_video.txt.20260930-120000.bak`. The editor retains the newest five recognized backups. It does not delete unrelated `.bak` files.

Choose Manage backups to inspect and restore a backup. Restoration validates the backup and creates a rollback backup of the current file first. Keep CS2 closed during restoration as well as during Apply.

## Preferences and upgrading from 1.1.0

Close the old application and replace its executable with version 2.0.0. Existing CS2 files and recognized editor backups remain usable. No installation or configuration conversion is required.

Preferences are stored at:

```text
%LOCALAPPDATA%\Softhe\CS2-ResEdit\v1\settings.json
```

The `v1` directory names the preference schema, not the application release. Version 2.0.0 keeps that schema and adds an optional theme field. It remembers the last account, selected configuration, theme, and up to five recent valid files. A missing or unknown theme defaults to Glacier.

Preference files under other product names or unversioned directories are not imported or modified. Preferences contain local account IDs and file paths, but no credentials.

## Diagnostics

Choose Diagnostics to view a support report. Copy copies the readable summary; Export JSON saves the structured report.

Reports include application and operating-system details, display and mode counts, Steam discovery counts, and configuration-format status. They exclude account names and identifiers, paths, configuration contents, and preference values. Exporting a report does not send it anywhere.

## Verify the download

The release is unsigned. Windows SmartScreen may warn when you run it. Download the executable and checksum sidecar into the same folder, then run this in PowerShell:

```powershell
$expected = (Get-Content .\CS2-ResEdit.exe.sha256 -Raw).Split()[0]
$actual = (Get-FileHash .\CS2-ResEdit.exe -Algorithm SHA256).Hash.ToLowerInvariant()
if ($actual -ne $expected) { throw 'The executable checksum does not match.' }
'Checksum verified.'
```

A matching checksum confirms that the file matches the release asset. It does not verify a publisher signature.

## Troubleshooting

### No account appears

Start Steam at least once, then choose Refresh. If discovery still fails, use Browse to open the account's configuration directly.

### Apply changes is disabled

Select a valid configuration and change a setting. Apply remains disabled when pending values match the loaded file or entered dimensions are invalid.

### The game restores previous settings

Close CS2 before applying changes. A running game may save its in-memory settings over the edited file when it exits.

### A change needs to be undone

Open Manage backups, inspect a backup's values, and restore it. Reset only discards changes that have not been applied.

### Two monitors have the same model name

Use the display ID and primary-display label to distinguish them. Model names depend on what Windows and the display driver report.

## Build from source

Install rustup and the toolchain specified in `rust-toolchain.toml`. From the repository root, run:

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
.\build\Publish.ps1
```

The workspace has two crates:

- `crates/core` contains configuration editing, backups, Steam discovery, display discovery, preferences, and diagnostics.
- `crates/app` contains the egui interface, themes, and native window integration.

The publish script stages the Windows x64 executable and checksum before replacing the files in `dist`. Close a running copy of the destination executable before publishing.

Release assets are `CS2-ResEdit.exe` and `CS2-ResEdit.exe.sha256`. CI checks formatting, lint, and tests on Windows and Linux, builds on Windows, and smoke-tests an identical copy of the packaged GUI with a pinned software OpenGL renderer. That renderer is a CI dependency and is not included in the release. Linux checks cover core behavior and conditional compilation; there is no supported Linux release.

A version tag must match the workspace version and the heading in `RELEASE_NOTES.md`. The release workflow uploads the tested Windows assets. `build/Sign-Release.ps1` supports optional manual Authenticode signing when a trusted publisher certificate is available. The current release process does not sign the executable.

See [the changelog](CHANGELOG.md), [the roadmap](ROADMAP.md), and the dated [code audit](CODE_AUDIT_2026-09-29.md) and [codebase analysis](PROJECT-ANALYSIS-2026-09-29.md) for change history and remaining work.

## License

CS2 ResEdit is licensed under [GPL-3.0-only](LICENSE).
