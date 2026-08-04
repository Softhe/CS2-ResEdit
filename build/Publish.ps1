[CmdletBinding()]
param(
    [string]$Configuration = 'Release',
    [string]$OutputDirectory = (Join-Path $PSScriptRoot '..\dist'),
    [switch]$SkipTests
)

$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$output = [IO.Path]::GetFullPath($OutputDirectory)
$project = Join-Path $root 'src\CS2ResEdit.Editor\CS2ResEdit.Editor.csproj'
$solution = Join-Path $root 'CS2-ResEdit.slnx'

if (-not $SkipTests) {
    & dotnet test $solution -c $Configuration
    if ($LASTEXITCODE -ne 0) { throw 'Tests failed.' }
}

if (-not (Test-Path -LiteralPath $output)) {
    [void][IO.Directory]::CreateDirectory($output)
}

& dotnet publish $project -c $Configuration -r win-x64 --self-contained true -o $output
if ($LASTEXITCODE -ne 0) { throw 'Publish failed.' }

$exe = Join-Path $output 'CS2-ResEdit.exe'
if (-not (Test-Path -LiteralPath $exe -PathType Leaf)) { throw 'Published executable was not found.' }
$hash = (Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash.ToLowerInvariant()
[IO.File]::WriteAllText(
    "$exe.sha256",
    "$hash  CS2-ResEdit.exe`n",
    [Text.UTF8Encoding]::new($false)
)

[pscustomobject]@{
    Executable = $exe
    Checksum = "$exe.sha256"
    Sha256 = $hash
    Size = (Get-Item -LiteralPath $exe).Length
}
