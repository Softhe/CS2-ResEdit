[CmdletBinding()]
param(
    [string[]]$HostPath,
    [switch]$KeepWorkspace
)

$ErrorActionPreference = 'Stop'
$repositoryRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$workspace = Join-Path ([IO.Path]::GetTempPath()) ('cs2-packaged-e2e-' + [guid]::NewGuid().ToString('N'))
$artifactDirectory = Join-Path $workspace 'artifacts'
$packageDirectory = Join-Path $workspace 'package'
$fixtureRoot = Join-Path $workspace 'fixture-steam'
$configPath = Join-Path $fixtureRoot 'userdata\424242\730\local\cfg\cs2_video.txt'
$diagnosticsPath = Join-Path $workspace 'diagnostics.json'
$isolatedProfile = Join-Path $workspace 'profile'
$settingsPath = Join-Path $isolatedProfile 'AppData\Local\Softhe\CS2-VideoConfig-Editor\settings.json'

function Invoke-IsolatedHost {
    param(
        [Parameter(Mandatory)][string]$Executable,
        [Parameter(Mandatory)][string[]]$Arguments
    )

    $savedEnvironment = @{
        LOCALAPPDATA = $env:LOCALAPPDATA
        APPDATA = $env:APPDATA
        USERPROFILE = $env:USERPROFILE
    }
    try {
        $env:LOCALAPPDATA = Join-Path $isolatedProfile 'AppData\Local'
        $env:APPDATA = Join-Path $isolatedProfile 'AppData\Roaming'
        $env:USERPROFILE = $isolatedProfile
        $output = @(& $Executable @Arguments 2>&1)
        if ($LASTEXITCODE -ne 0) {
            throw "Host '$Executable' exited with code $LASTEXITCODE.`n$($output -join [Environment]::NewLine)"
        }
        return $output
    } finally {
        $env:LOCALAPPDATA = $savedEnvironment.LOCALAPPDATA
        $env:APPDATA = $savedEnvironment.APPDATA
        $env:USERPROFILE = $savedEnvironment.USERPROFILE
    }
}

try {
    [void][IO.Directory]::CreateDirectory((Split-Path $configPath))
    $fixture = @'
"VideoConfig"
{
    "setting.defaultres"        "1024"
    "setting.defaultresheight"  "768"
    "setting.aspectratiomode"   "0"
}
'@ -replace "`n", "`r`n"
    [IO.File]::WriteAllText($configPath, $fixture, (New-Object Text.UTF8Encoding($false)))
    $originalHash = (Get-FileHash $configPath -Algorithm SHA256).Hash

    [void](& (Join-Path $PSScriptRoot 'Build-Release.ps1') -OutputDirectory $artifactDirectory)
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    [IO.Compression.ZipFile]::ExtractToDirectory(
        (Join-Path $artifactDirectory 'CS2-VideoConfig-Editor.zip'),
        $packageDirectory
    )

    $launcher = Join-Path $packageDirectory 'CS2-VideoConfig-Editor.ps1'
    $expectedModules = @(
        'Cs2.VideoConfig.psm1'
        'Cs2.Steam.psm1'
        'Cs2.Preferences.psm1'
        'Cs2.Diagnostics.psm1'
    )
    foreach ($module in $expectedModules) {
        $modulePath = Join-Path $packageDirectory "modules\$module"
        if (-not (Test-Path -LiteralPath $modulePath -PathType Leaf)) {
            throw "Packaged module is missing: $module"
        }
    }

    if (-not $HostPath -or $HostPath.Count -eq 0) {
        $HostPath = @()
        $desktopHost = Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe'
        if (Test-Path -LiteralPath $desktopHost -PathType Leaf) { $HostPath += $desktopHost }
        $pwsh = Get-Command pwsh.exe -ErrorAction SilentlyContinue
        if ($pwsh) { $HostPath += $pwsh.Source }
        $HostPath = @($HostPath | Select-Object -Unique)
    }
    if (-not $HostPath -or $HostPath.Count -eq 0) {
        throw 'No PowerShell host was available for the packaged application test.'
    }

    $hostResults = @()
    foreach ($executable in $HostPath) {
        if (-not (Test-Path -LiteralPath $executable -PathType Leaf)) {
            throw "PowerShell host was not found: $executable"
        }

        # Reset the same isolated fixture so every host performs the complete flow.
        [IO.File]::WriteAllText($configPath, $fixture, (New-Object Text.UTF8Encoding($false)))
        if (Test-Path -LiteralPath $diagnosticsPath) {
            Remove-Item -LiteralPath $diagnosticsPath -Force
        }

        [void](Invoke-IsolatedHost $executable @(
            '-NoLogo', '-NoProfile', '-NonInteractive', '-File', $launcher,
            '-FilePath', $configPath, '-Preset', '1280x720', '-NoBackup', '-Silent'
        ))
        $updatedText = [IO.File]::ReadAllText($configPath)
        if ($updatedText -notmatch '"setting\.defaultres"\s+"1280"' -or
            $updatedText -notmatch '"setting\.defaultresheight"\s+"720"' -or
            $updatedText -notmatch '"setting\.aspectratiomode"\s+"1"') {
            throw "Packaged preset update did not produce the expected 1280x720 16:9 configuration in '$executable'."
        }
        if ((Get-FileHash $configPath -Algorithm SHA256).Hash -eq $originalHash) {
            throw "Packaged preset update did not change the fixture in '$executable'."
        }
        if (@(Get-ChildItem (Split-Path $configPath) -Filter '*.bak' -Force).Count -ne 0) {
            throw "Packaged -NoBackup run created a backup in '$executable'."
        }

        [void](Invoke-IsolatedHost $executable @(
            '-NoLogo', '-NoProfile', '-NonInteractive', '-File', $launcher,
            '-SteamRoot', $fixtureRoot, '-FilePath', $configPath,
            '-ExportDiagnostics', $diagnosticsPath, '-Silent'
        ))
        if (-not (Test-Path -LiteralPath $diagnosticsPath -PathType Leaf)) {
            throw "Packaged diagnostic export was not created in '$executable'."
        }
        $report = Get-Content -LiteralPath $diagnosticsPath -Raw | ConvertFrom-Json
        if ($report.SchemaVersion -ne 1 -or $report.SelectedConfig.Width -ne 1280 -or
            $report.SelectedConfig.Height -ne 720 -or -not $report.SelectedConfig.Readable) {
            throw "Packaged diagnostic export had unexpected content in '$executable'."
        }
        $rawReport = [IO.File]::ReadAllText($diagnosticsPath)
        if ($rawReport.Contains($fixtureRoot) -or $rawReport.Contains('424242')) {
            throw "Packaged diagnostic export leaked isolated fixture identifiers in '$executable'."
        }
        if (Test-Path -LiteralPath $settingsPath) {
            throw "A non-GUI packaged run wrote GUI preferences in '$executable'."
        }

        $hostResults += [pscustomobject]@{
            Host = $executable
            PowerShellVersion = $report.Runtime.PSVersion
            PresetApplied = $true
            DiagnosticsExported = $true
            PreferencesCreated = $false
        }
    }

    [pscustomobject]@{
        Passed = $true
        PackageFileCount = 1 + $expectedModules.Count
        Hosts = $hostResults
        Workspace = if ($KeepWorkspace) { $workspace } else { $null }
    }
} finally {
    if (-not $KeepWorkspace -and (Test-Path -LiteralPath $workspace)) {
        Remove-Item -LiteralPath $workspace -Recurse -Force
    }
}
