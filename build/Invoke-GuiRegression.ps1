[CmdletBinding()]
param(
    [switch]$Interactive
)

$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$testPath = Join-Path $root 'tests\Gui-Regression.Tests.ps1'

$configuration = New-PesterConfiguration
$configuration.Run.Path = $testPath
$configuration.Run.PassThru = $true
$configuration.Output.Verbosity = 'Detailed'
$result = Invoke-Pester -Configuration $configuration
if ($result.FailedCount -gt 0) {
    throw "GUI regression suite failed: $($result.FailedCount) test(s) failed."
}

if (-not $Interactive) {
    return
}

if ($env:OS -ne 'Windows_NT') {
    throw 'The interactive GUI smoke check requires Windows.'
}

$sandbox = Join-Path ([IO.Path]::GetTempPath()) ("cs2-gui-smoke-{0}" -f [guid]::NewGuid().ToString('N'))
$configPath = Join-Path $sandbox 'Steam\userdata\424242\730\local\cfg\cs2_video.txt'
$configDirectory = [IO.Path]::GetDirectoryName($configPath)
[void][IO.Directory]::CreateDirectory($configDirectory)
$fixture = @'
"VideoConfig"
{
    "setting.defaultres"        "1920"
    "setting.defaultresheight"  "1080"
    "setting.aspectratiomode"   "1"
}
'@
[IO.File]::WriteAllText($configPath, $fixture, (New-Object Text.UTF8Encoding($false)))

Write-Host ''
Write-Host 'Interactive smoke check:' -ForegroundColor Cyan
Write-Host '  1. Resize across wide and narrow layouts.'
Write-Host '  2. Press Tab through every input and action.'
Write-Host '  3. Confirm Enter applies only after a change and Escape closes safely.'
Write-Host '  4. Check Windows scaling or High Contrast if those modes are available.'
Write-Host 'Close the editor to finish. The configuration is an isolated temporary fixture.'

try {
    $hostExecutable = (Get-Process -Id $PID).Path
    $editorPath = Join-Path $root 'CS2-VideoConfig-Editor.ps1'
    $oldLocalAppData = $env:LOCALAPPDATA
    try {
        $env:LOCALAPPDATA = Join-Path $sandbox 'LocalAppData'
        & $hostExecutable -NoProfile -STA -File $editorPath -FilePath $configPath
        if ($LASTEXITCODE -ne 0) {
            throw "Interactive editor exited with code $LASTEXITCODE."
        }
    } finally {
        $env:LOCALAPPDATA = $oldLocalAppData
    }
} finally {
    $resolvedSandbox = [IO.Path]::GetFullPath($sandbox)
    $resolvedTemp = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
    if ($resolvedSandbox.StartsWith($resolvedTemp, [StringComparison]::OrdinalIgnoreCase) -and
        (Split-Path -Leaf $resolvedSandbox) -like 'cs2-gui-smoke-*') {
        Remove-Item -LiteralPath $resolvedSandbox -Recurse -Force -ErrorAction SilentlyContinue
    }
}
