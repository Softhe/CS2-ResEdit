BeforeAll {
    $scriptPath = Join-Path $PSScriptRoot '..\CS2-VideoConfig-Editor.ps1'
    . $scriptPath

    function Write-TestConfig {
        param(
            [string]$Path,
            [int]$Width = 1920,
            [int]$Height = 1080,
            [string]$Mode = '1',
            [switch]$DuplicateWidth,
            [switch]$MissingHeight,
            [Text.Encoding]$Encoding = (New-Object Text.UTF8Encoding($false))
        )
        $heightLine = if ($MissingHeight) { '' } else { "    `"setting.defaultresheight`"  `"$Height`"`r`n" }
        $duplicateLine = if ($DuplicateWidth) { "    `"setting.defaultres`"        `"$Width`"`r`n" } else { '' }
        $text = "`"VideoConfig`"`r`n{`r`n    `"setting.defaultres`"        `"$Width`"`r`n$duplicateLine$heightLine    `"setting.aspectratiomode`"   `"$Mode`"`r`n}`r`n"
        [IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($Path)) | Out-Null
        [IO.File]::WriteAllText($Path, $text, $Encoding)
    }
}

Describe 'Application metadata and import behavior' {
    It 'exposes version 2.0.0 when dot-sourced' {
        $script:ApplicationVersion | Should -Be '2.0.0'
        Get-Command Invoke-Cs2VideoConfigEditor | Should -Not -BeNullOrEmpty
    }
}

Describe 'Steam account discovery' {
    It 'converts a 32-bit account ID to SteamID64' {
        ConvertTo-SteamId64 2421650 | Should -Be '76561197962687378'
    }

    It 'reads PersonaName from a CRLF loginusers.vdf' {
        $root = Join-Path $TestDrive 'Steam'
        $configPath = Join-Path $root 'userdata\424242\730\local\cfg\cs2_video.txt'
        Write-TestConfig $configPath
        $steamId64 = ConvertTo-SteamId64 424242
        $loginUsers = @(
            '"users"'
            '{'
            "    `"$steamId64`""
            '    {'
            '        "AccountName"   "fixture"'
            '        "PersonaName"   "Fixture Persona"'
            '        "MostRecent"    "1"'
            '    }'
            '}'
        ) -join "`r`n"
        $loginUsers += "`r`n"
        [IO.Directory]::CreateDirectory((Join-Path $root 'config')) | Out-Null
        [IO.File]::WriteAllText((Join-Path $root 'config\loginusers.vdf'), $loginUsers, (New-Object Text.UTF8Encoding($false)))
        $accounts = @(Get-SteamAccounts @($root))
        $accounts.Count | Should -Be 1
        $accounts[0].PersonaName | Should -Be 'Fixture Persona'
        $accounts[0].SteamId64 | Should -Be $steamId64
    }

    It 'keeps an explicit Steam root available for account refresh' {
        $root = Join-Path $TestDrive 'CustomSteam'
        $configPath = Join-Path $root 'userdata\515151\730\local\cfg\cs2_video.txt'
        Write-TestConfig $configPath
        $roots = @(Get-SteamRoots $root)
        $accounts = @(Get-SteamAccounts -Roots $roots)
        $roots | Should -HaveCount 1
        $roots[0] | Should -Be ([IO.Path]::GetFullPath($root))
        $accounts | Should -HaveCount 1
        $accounts[0].AccountId | Should -Be '515151'
    }
}

Describe 'Configuration parsing and state' {
    It 'rejects a missing required entry' {
        $path = Join-Path $TestDrive 'missing.txt'
        Write-TestConfig $path -MissingHeight
        { Get-CurrentConfig $path } | Should -Throw "*setting.defaultresheight*"
    }

    It 'rejects duplicate entries' {
        $path = Join-Path $TestDrive 'duplicate.txt'
        Write-TestConfig $path -DuplicateWidth
        { Get-CurrentConfig $path } | Should -Throw "*setting.defaultres*found 2*"
    }

    It 'detects pending changes and no-change state' {
        $current = [pscustomobject]@{ Width = 1920; Height = 1080; Mode = '1' }
        (Get-Cs2PendingState $current $current $true).AlreadyActive | Should -BeTrue
        $pending = [pscustomobject]@{ Width = 1600; Height = 900; Mode = '0' }
        (Get-Cs2PendingState $current $pending $true).CanApply | Should -BeTrue
        (Get-Cs2PendingState $current $pending $false).CanApply | Should -BeFalse
    }

    It 'explains a stretched aspect mismatch without changing it' {
        Get-Cs2AspectGuidance 1600 900 '0' | Should -Be 'Dimensions are 16:9; aspect mode remains 4:3 / 5:4.'
        Get-Cs2AspectGuidance 1600 900 '1' | Should -BeNullOrEmpty
    }

    It 'uses Windows system colors for High Contrast' {
        Add-Type -AssemblyName System.Drawing
        $theme = Get-Cs2UiTheme $true
        $theme.Background | Should -Be ([Drawing.SystemColors]::Window)
        $theme.Text | Should -Be ([Drawing.SystemColors]::WindowText)
        $theme.Primary | Should -Be ([Drawing.SystemColors]::Highlight)
    }

    It 'scales the responsive breakpoint at common Windows DPI levels' {
        Get-Cs2ResponsiveBreakpoint 96 | Should -Be 900
        Get-Cs2ResponsiveBreakpoint 120 | Should -Be 1125
        Get-Cs2ResponsiveBreakpoint 144 | Should -Be 1350
        Get-Cs2ResponsiveBreakpoint 192 | Should -Be 1800
    }

    It 'reflows the same controls between wide and stacked layouts' {
        Initialize-Cs2GraphicalInterface
        $form = New-Object Windows.Forms.Form
        $body = New-Object Windows.Forms.TableLayoutPanel
        $accountCard = New-Object Windows.Forms.Panel
        $displayCard = New-Object Windows.Forms.Panel
        $status = New-Object Windows.Forms.Label
        $state = [pscustomobject]@{ IsWide = $false }
        try {
            $form.Controls.Add($body)
            $body.Controls.Add($accountCard)
            $body.Controls.Add($displayCard)
            $body.Controls.Add($status)
            $form.ClientSize = New-Object Drawing.Size(1200, 700)
            Set-Cs2ResponsiveEditorLayout $form $body $accountCard $displayCard $status $state
            $state.IsWide | Should -BeTrue
            $body.ColumnCount | Should -Be 2
            $body.GetColumnSpan($status) | Should -Be 2

            $form.ClientSize = New-Object Drawing.Size(700, 700)
            Set-Cs2ResponsiveEditorLayout $form $body $accountCard $displayCard $status $state
            $state.IsWide | Should -BeFalse
            $body.ColumnCount | Should -Be 1
            $body.RowCount | Should -Be 3
        } finally {
            $form.Dispose()
        }
    }
}

Describe 'Atomic updates and backups' {
    It 'updates a preset while preserving UTF-8 without BOM and CRLF' {
        $path = Join-Path $TestDrive 'preserve.txt'
        Write-TestConfig $path -Width 1024 -Height 768 -Mode '0'
        $result = Update-VideoConfig $path (New-Resolution 1920 1080 '1') $true
        $result.Changed | Should -BeTrue
        Test-Path -LiteralPath $result.BackupPath | Should -BeTrue
        $text = [IO.File]::ReadAllText($path)
        $text | Should -Not -Match '(?<!\r)\n'
        $bytes = [IO.File]::ReadAllBytes($path)
        ($bytes.Length -ge 3 -and $bytes[0] -eq 0xEF -and $bytes[1] -eq 0xBB -and $bytes[2] -eq 0xBF) | Should -BeFalse
        (Get-CurrentConfig $path).Width | Should -Be 1920
    }

    It 'preserves UTF-16 LE encoding' {
        $path = Join-Path $TestDrive 'utf16.txt'
        Write-TestConfig $path -Encoding (New-Object Text.UnicodeEncoding($false, $true))
        [void](Update-VideoConfig $path (New-Resolution 1280 720 '1') $false)
        $bytes = [IO.File]::ReadAllBytes($path)
        $bytes[0] | Should -Be 0xFF
        $bytes[1] | Should -Be 0xFE
    }

    It 'does not write or create a backup for an active selection' {
        $path = Join-Path $TestDrive 'unchanged.txt'
        Write-TestConfig $path
        $before = (Get-FileHash $path -Algorithm SHA256).Hash
        $result = Update-VideoConfig $path (New-Resolution 1920 1080 '1') $true
        $result.Changed | Should -BeFalse
        (Get-FileHash $path -Algorithm SHA256).Hash | Should -Be $before
        @(Get-Cs2BackupFiles $path).Count | Should -Be 0
    }

    It 'leaves no temporary file after a rejected malformed update' {
        $path = Join-Path $TestDrive 'malformed-update.txt'
        Write-TestConfig $path -MissingHeight
        { Update-VideoConfig $path (New-Resolution 1280 720 '1') $true } | Should -Throw
        @(Get-ChildItem $TestDrive -Filter '.cs2-video-*.tmp' -Force).Count | Should -Be 0
    }

    It 'fails safely when the active file is exclusively locked' {
        $path = Join-Path $TestDrive 'locked.txt'
        Write-TestConfig $path
        $before = (Get-FileHash $path -Algorithm SHA256).Hash
        $lock = [IO.File]::Open($path, 'Open', 'ReadWrite', 'None')
        try {
            { Update-VideoConfig $path (New-Resolution 1280 720 '1') $true } | Should -Throw
        } finally {
            $lock.Dispose()
        }
        (Get-FileHash $path -Algorithm SHA256).Hash | Should -Be $before
        @(Get-ChildItem $TestDrive -Filter '.cs2-video-*.tmp' -Force).Count | Should -Be 0
    }

    It 'does not modify a read-only configuration' {
        $path = Join-Path $TestDrive 'read-only.txt'
        Write-TestConfig $path
        $before = (Get-FileHash $path -Algorithm SHA256).Hash
        (Get-Item -LiteralPath $path).IsReadOnly = $true
        try {
            { Update-VideoConfig $path (New-Resolution 1280 720 '1') $true } | Should -Throw
        } finally {
            (Get-Item -LiteralPath $path).IsReadOnly = $false
        }
        (Get-FileHash $path -Algorithm SHA256).Hash | Should -Be $before
        @(Get-ChildItem $TestDrive -Filter '.cs2-video-*.tmp' -Force).Count | Should -Be 0
    }
}

Describe 'Backup discovery, restore, and retention' {
    It 'ignores unrelated backup files and marks malformed editor backups invalid' {
        $path = Join-Path $TestDrive 'history.txt'
        Write-TestConfig $path
        Write-TestConfig "$path.20260101-010101.bak" -Width 1280 -Height 720
        [IO.File]::WriteAllText("$path.20260102-010101.bak", 'broken')
        [IO.File]::WriteAllText((Join-Path $TestDrive 'manual.bak'), 'unrelated')
        $backups = @(Get-Cs2BackupFiles $path)
        $backups.Count | Should -Be 2
        @($backups | Where-Object Valid).Count | Should -Be 1
        @($backups | Where-Object { -not $_.Valid }).Count | Should -Be 1
    }

    It 'restores atomically and creates a rollback backup' {
        $path = Join-Path $TestDrive 'restore.txt'
        Write-TestConfig $path -Width 1920 -Height 1080 -Mode '1'
        $backup = "$path.20260101-010101.bak"
        Write-TestConfig $backup -Width 1280 -Height 960 -Mode '0'
        $result = Restore-Cs2Backup $path $backup
        $result.Changed | Should -BeTrue
        Test-Path -LiteralPath $backup | Should -BeTrue
        Test-Path -LiteralPath $result.BackupPath | Should -BeTrue
        (Get-CurrentConfig $path).Width | Should -Be 1280
        @(Get-ChildItem $TestDrive -Filter '.cs2-video-*.tmp' -Force).Count | Should -Be 0
    }

    It 'does not restore identical settings' {
        $path = Join-Path $TestDrive 'same-restore.txt'
        Write-TestConfig $path
        $backup = "$path.20260101-010101.bak"
        Write-TestConfig $backup
        (Restore-Cs2Backup $path $backup).Changed | Should -BeFalse
        @(Get-Cs2BackupFiles $path).Count | Should -Be 1
    }

    It 'fails safely when the active file is locked during restore' {
        $path = Join-Path $TestDrive 'locked-restore.txt'
        Write-TestConfig $path
        $backup = "$path.20260101-010101.bak"
        Write-TestConfig $backup -Width 1280 -Height 960 -Mode '0'
        $before = (Get-FileHash $path -Algorithm SHA256).Hash
        $lock = [IO.File]::Open($path, 'Open', 'ReadWrite', 'None')
        try {
            { Restore-Cs2Backup $path $backup } | Should -Throw
        } finally {
            $lock.Dispose()
        }
        (Get-FileHash $path -Algorithm SHA256).Hash | Should -Be $before
        Test-Path -LiteralPath $backup | Should -BeTrue
        @(Get-ChildItem $TestDrive -Filter '.cs2-video-*.tmp' -Force).Count | Should -Be 0
    }

    It 'retains only the newest five editor backups' {
        $path = Join-Path $TestDrive 'retention.txt'
        Write-TestConfig $path
        1..7 | ForEach-Object {
            Write-TestConfig ("$path.2026010$_-010101.bak") -Width (1000 + $_) -Height 700
        }
        [IO.File]::WriteAllText((Join-Path $TestDrive 'keep-me.bak'), 'unrelated')
        $removed = @(Remove-Cs2OldBackups $path 5)
        $removed.Count | Should -Be 2
        @(Get-Cs2BackupFiles $path).Count | Should -Be 5
        Test-Path (Join-Path $TestDrive 'keep-me.bak') | Should -BeTrue
    }

    It 'uses a numeric suffix when a timestamped backup name already exists' {
        $path = Join-Path $TestDrive 'collision.txt'
        Write-TestConfig $path
        $first = New-Cs2BackupPath $path
        [IO.File]::WriteAllText($first, 'occupied')
        $second = New-Cs2BackupPath $path
        $second | Should -Be ($first -replace '\.bak$', '-1.bak')
    }

    It 'reports a retention warning without hiding a successful update' {
        $path = Join-Path $TestDrive 'retention-warning.txt'
        Write-TestConfig $path
        1..5 | ForEach-Object {
            Write-TestConfig ("$path.2026010$_-010101.bak") -Width (1000 + $_) -Height 700
        }
        $oldest = "$path.20260101-010101.bak"
        $lock = [IO.File]::Open($oldest, 'Open', 'Read', 'None')
        try {
            $result = Update-VideoConfig $path (New-Resolution 1280 720 '1') $true
        } finally {
            $lock.Dispose()
        }
        $result.Changed | Should -BeTrue
        $result.RetentionWarning | Should -Match 'retention could not be completed'
        (Get-CurrentConfig $path).Width | Should -Be 1280
        @(Get-Cs2BackupFiles $path).Count | Should -Be 6
    }
}

Describe 'GUI preferences' {
    It 'writes schema 1 atomically, deduplicates paths, and keeps five' {
        $settings = Join-Path $TestDrive 'prefs\settings.json'
        $paths = 1..6 | ForEach-Object {
            $path = Join-Path $TestDrive "config$_\cs2_video.txt"
            Write-TestConfig $path
            $path
        }
        $saved = Save-Cs2Preferences -LastAccountId '424242' -RecentConfigPaths (@($paths[0], $paths[0]) + $paths[1..5]) -Path $settings
        $saved.RecentConfigPaths.Count | Should -Be 5
        $loaded = Read-Cs2Preferences $settings
        $loaded.LastAccountId | Should -Be '424242'
        $loaded.RecentConfigPaths.Count | Should -Be 5
        @(Get-ChildItem (Split-Path $settings) -Filter '.settings-*.tmp' -Force).Count | Should -Be 0
    }

    It 'drops missing recent files on load' {
        $settings = Join-Path $TestDrive 'missing-prefs.json'
        $payload = @{ SchemaVersion = 1; LastAccountId = '1'; RecentConfigPaths = @((Join-Path $TestDrive 'missing.txt')) } | ConvertTo-Json
        [IO.File]::WriteAllText($settings, $payload)
        (Read-Cs2Preferences $settings).RecentConfigPaths.Count | Should -Be 0
    }

    It 'returns defaults and a warning for corrupt JSON' {
        $settings = Join-Path $TestDrive 'corrupt-prefs.json'
        [IO.File]::WriteAllText($settings, '{not json')
        $loaded = Read-Cs2Preferences $settings
        $loaded.SchemaVersion | Should -Be 1
        $loaded.RecentConfigPaths.Count | Should -Be 0
        $loaded.Warning | Should -Match 'could not be loaded'
    }

    It 'returns defaults and a warning for an unsupported schema' {
        $settings = Join-Path $TestDrive 'future-prefs.json'
        $payload = @{ SchemaVersion = 99; LastAccountId = '1'; RecentConfigPaths = @() } | ConvertTo-Json
        [IO.File]::WriteAllText($settings, $payload)
        $loaded = Read-Cs2Preferences $settings
        $loaded.SchemaVersion | Should -Be 1
        $loaded.Warning | Should -Match 'Unsupported preference schema'
    }

    It 'fails cleanly when the settings directory is not writable as a directory' {
        $blocker = Join-Path $TestDrive 'settings-blocker'
        [IO.File]::WriteAllText($blocker, 'file blocks directory creation')
        $settings = Join-Path $blocker 'settings.json'
        { Save-Cs2Preferences -LastAccountId '1' -RecentConfigPaths @() -Path $settings } | Should -Throw
        @(Get-ChildItem $TestDrive -Filter '.settings-*.tmp' -Recurse -Force -ErrorAction SilentlyContinue).Count | Should -Be 0
    }

    It 'deduplicates discovered and recent account choices by path' {
        $path = Join-Path $TestDrive 'choice\cs2_video.txt'
        Write-TestConfig $path
        $account = [pscustomobject]@{ ConfigPath = $path; AccountId = '7'; PersonaName = 'Known' }
        $other = Join-Path $TestDrive 'other\cs2_video.txt'
        Write-TestConfig $other
        $choices = @(Merge-Cs2AccountChoices @($account) @($path.ToUpperInvariant(), $other))
        $choices | Should -HaveCount 2
        @($choices | Where-Object ConfigPath -eq $path).Count | Should -Be 1
        @($choices | Where-Object ConfigPath -eq $other).Count | Should -Be 1
    }
}

Describe 'Release tooling' {
    It 'rejects a build version that does not match the application' {
        $buildScript = Join-Path $PSScriptRoot '..\build\Build-Release.ps1'
        $output = Join-Path $TestDrive 'mismatch-build'
        { & $buildScript -Version '9.9.9' -OutputDirectory $output } | Should -Throw "*does not match source version*"
        Test-Path (Join-Path $output 'CS2-VideoConfig-Editor.zip') | Should -BeFalse
    }

    It 'accepts only a tag matching the application version' {
        $metadataScript = Join-Path $PSScriptRoot '..\build\Test-ReleaseMetadata.ps1'
        { & $metadataScript -Tag 'v9.9.9' -RefType 'tag' } | Should -Throw "*does not match source version 'v2.0.0'*"
        { & $metadataScript -Tag 'v2.0.0' -RefType 'branch' } | Should -Throw "*requires a tag ref*"
        $metadata = & $metadataScript -Tag 'v2.0.0' -RefType 'tag'
        $metadata.Version | Should -Be '2.0.0'
    }
}

Describe 'Non-GUI preference isolation' {
    It 'does not create GUI settings in preset or account-listing modes' {
        $config = Join-Path $TestDrive 'isolated-config.txt'
        Write-TestConfig $config
        $steamRoot = Join-Path $TestDrive 'isolated-steam'
        Write-TestConfig (Join-Path $steamRoot 'userdata\123\730\local\cfg\cs2_video.txt')
        $isolatedLocalAppData = Join-Path $TestDrive 'isolated-localappdata'
        $hostExecutable = if ($PSVersionTable.PSEdition -eq 'Desktop') {
            Join-Path $PSHOME 'powershell.exe'
        } else {
            Join-Path $PSHOME 'pwsh.exe'
        }
        $previousLocalAppData = $env:LOCALAPPDATA
        try {
            $env:LOCALAPPDATA = $isolatedLocalAppData
            & $hostExecutable -NoLogo -NoProfile -NonInteractive -File $scriptPath -FilePath $config -Preset '1280x720' -NoBackup -Silent
            $LASTEXITCODE | Should -Be 0
            & $hostExecutable -NoLogo -NoProfile -NonInteractive -File $scriptPath -SteamRoot $steamRoot -ListAccounts | Out-Null
            $LASTEXITCODE | Should -Be 0
        } finally {
            $env:LOCALAPPDATA = $previousLocalAppData
        }
        Test-Path (Join-Path $isolatedLocalAppData 'Softhe\CS2-VideoConfig-Editor\settings.json') | Should -BeFalse
    }
}
