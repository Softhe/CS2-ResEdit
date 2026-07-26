BeforeAll {
    $diagnosticScriptPath = Join-Path $PSScriptRoot '..\CS2-VideoConfig-Editor.ps1'
    . $diagnosticScriptPath

    function Write-DiagnosticTestConfig {
        param(
            [string]$Path,
            [int]$Width = 1920,
            [int]$Height = 1080,
            [string]$Mode = '1',
            [switch]$Malformed
        )

        [void][IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($Path))
        $text = if ($Malformed) {
            "`"VideoConfig`"`r`n{`r`n    `"setting.defaultres`" `"broken`"`r`n}`r`n"
        } else {
            "`"VideoConfig`"`r`n{`r`n    `"setting.defaultres`" `"$Width`"`r`n    `"setting.defaultresheight`" `"$Height`"`r`n    `"setting.aspectratiomode`" `"$Mode`"`r`n}`r`n"
        }
        [IO.File]::WriteAllText($Path, $text, (New-Object Text.UTF8Encoding($false)))
    }
}

Describe 'Privacy-safe diagnostic export' {
    It 'writes schema 1 JSON without personal identifiers, raw roots, or a BOM' {
        $steamRoot = Join-Path $TestDrive 'Private Steam Root'
        $configPath = Join-Path $steamRoot 'userdata\424242\730\local\cfg\cs2_video.txt'
        Write-DiagnosticTestConfig $configPath
        $account = [pscustomobject]@{
            PersonaName = 'Very Private Persona'
            AccountName = 'private-login'
            AccountId = '424242'
            SteamId64 = '76561197960689970'
            HasConfig = $true
            MostRecent = $true
            ConfigPath = $configPath
        }
        $outputPath = Join-Path $TestDrive 'reports\diagnostics.json'

        $writtenPath = Export-Cs2Diagnostics `
            -Path $outputPath `
            -ApplicationVersion '2.0.0' `
            -SteamRoots @($steamRoot) `
            -Accounts @($account) `
            -SelectedPath $configPath

        $writtenPath | Should -Be ([IO.Path]::GetFullPath($outputPath))
        $bytes = [IO.File]::ReadAllBytes($outputPath)
        ($bytes.Length -ge 3 -and $bytes[0] -eq 0xEF -and $bytes[1] -eq 0xBB -and $bytes[2] -eq 0xBF) | Should -BeFalse

        $raw = [IO.File]::ReadAllText($outputPath)
        $raw | Should -Not -Match ([regex]::Escape($steamRoot))
        $raw | Should -Not -Match 'Very Private Persona'
        $raw | Should -Not -Match 'private-login'
        $raw | Should -Not -Match '424242'
        $raw | Should -Not -Match '76561197960689970'

        $report = $raw | ConvertFrom-Json
        $report.SchemaVersion | Should -Be 1
        $report.Privacy.PathsAnonymized | Should -BeTrue
        $report.Privacy.PersonalIdentifiersIncluded | Should -BeFalse
        $report.Discovery.Accounts[0].Alias | Should -Be 'Account 1'
        $report.Discovery.Accounts[0].ConfigPath | Should -Match '^<STEAM_ROOT_1>\\userdata\\<ACCOUNT_ID>\\'
        $report.SelectedConfig.Readable | Should -BeTrue
        $report.SelectedConfig.Width | Should -Be 1920
        $report.SelectedConfig.Encoding | Should -Be 'utf-8'
    }

    It 'uses stable aliases while keeping distinct unknown paths distinct' {
        $context = New-Cs2DiagnosticContext
        $first = ConvertTo-Cs2DiagnosticPath $context 'D:\Private\one\cs2_video.txt'
        $firstAgain = ConvertTo-Cs2DiagnosticPath $context 'D:\Private\one\cs2_video.txt'
        $second = ConvertTo-Cs2DiagnosticPath $context 'E:\Elsewhere\two\cs2_video.txt'
        $first | Should -Be $firstAgain
        $first | Should -Not -Be $second
        $first | Should -Be '<CUSTOM_PATH_1>\cs2_video.txt'
        $second | Should -Be '<CUSTOM_PATH_2>\cs2_video.txt'
    }

    It 'sanitizes validation failures and overwrites atomically without temp residue' {
        $steamRoot = Join-Path $TestDrive 'SecretRoot'
        $configPath = Join-Path $steamRoot 'userdata\515151\730\local\cfg\cs2_video.txt'
        Write-DiagnosticTestConfig $configPath -Malformed
        $account = [pscustomobject]@{
            PersonaName = 'Hidden User'
            AccountName = 'hidden-login'
            AccountId = '515151'
            SteamId64 = '76561197960780879'
            HasConfig = $true
            MostRecent = $false
            ConfigPath = $configPath
        }
        $outputPath = Join-Path $TestDrive 'overwrite.json'
        [IO.File]::WriteAllText($outputPath, 'old')

        1..2 | ForEach-Object {
            [void](Export-Cs2Diagnostics `
                -Path $outputPath `
                -ApplicationVersion '2.0.0' `
                -SteamRoots @($steamRoot) `
                -Accounts @($account) `
                -SelectedPath $configPath)
        }

        $raw = [IO.File]::ReadAllText($outputPath)
        $raw | Should -Not -Match ([regex]::Escape($steamRoot))
        $raw | Should -Not -Match 'Hidden User|hidden-login|515151|76561197960780879'
        $report = $raw | ConvertFrom-Json
        $report.SelectedConfig.Readable | Should -BeFalse
        $report.SelectedConfig.ValidationError | Should -Match 'Cannot convert|Expected exactly one'
        @(Get-ChildItem $TestDrive -Filter '.diagnostics-*.tmp' -Recurse -Force).Count | Should -Be 0
    }

    It 'keeps only the newest twenty in-memory operation events' {
        $context = New-Cs2DiagnosticContext
        1..25 | ForEach-Object {
            [void](Add-Cs2DiagnosticEvent -Context $context -Kind 'Test' -Outcome 'Info' -Code "event-$_")
        }
        $context.Operations.Count | Should -Be 20
        $context.Operations[0].Code | Should -Be 'event-6'
        $context.Operations[-1].Code | Should -Be 'event-25'
    }

    It 'runs in a fresh process without changing config, backups, or GUI preferences' {
        $steamRoot = Join-Path $TestDrive 'isolated-steam'
        $configPath = Join-Path $steamRoot 'userdata\616161\730\local\cfg\cs2_video.txt'
        Write-DiagnosticTestConfig $configPath
        $beforeHash = (Get-FileHash $configPath -Algorithm SHA256).Hash
        $outputPath = Join-Path $TestDrive 'isolated-report.json'
        $isolatedLocalAppData = Join-Path $TestDrive 'isolated-localappdata'
        $hostExecutable = if ($PSVersionTable.PSEdition -eq 'Desktop') {
            Join-Path $PSHOME 'powershell.exe'
        } else {
            Join-Path $PSHOME 'pwsh.exe'
        }
        $previousLocalAppData = $env:LOCALAPPDATA
        try {
            $env:LOCALAPPDATA = $isolatedLocalAppData
            & $hostExecutable -NoLogo -NoProfile -NonInteractive -File $diagnosticScriptPath `
                -SteamRoot $steamRoot `
                -FilePath $configPath `
                -ExportDiagnostics $outputPath `
                -Silent
            $LASTEXITCODE | Should -Be 0
        } finally {
            $env:LOCALAPPDATA = $previousLocalAppData
        }

        Test-Path -LiteralPath $outputPath -PathType Leaf | Should -BeTrue
        (Get-FileHash $configPath -Algorithm SHA256).Hash | Should -Be $beforeHash
        @(Get-ChildItem ([IO.Path]::GetDirectoryName($configPath)) -Filter '*.bak' -Force).Count | Should -Be 0
        Test-Path (Join-Path $isolatedLocalAppData 'Softhe\CS2-VideoConfig-Editor\settings.json') | Should -BeFalse
    }
}
