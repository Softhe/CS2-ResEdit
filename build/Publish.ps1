[CmdletBinding()]
param(
    [string]$Target = 'x86_64-pc-windows-msvc',
    [string]$OutputDirectory,
    [switch]$SkipTests
)

$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
if (-not $OutputDirectory) { $OutputDirectory = Join-Path $root 'dist' }
$output = [IO.Path]::GetFullPath($OutputDirectory)
if ($Target -ne 'x86_64-pc-windows-msvc') {
    throw 'Publish.ps1 supports only the x86_64-pc-windows-msvc portable executable.'
}

if (-not $SkipTests) {
    & cargo test --manifest-path (Join-Path $root 'Cargo.toml') --workspace --locked
    if ($LASTEXITCODE -ne 0) { throw 'Tests failed.' }
}

if (-not (Test-Path -LiteralPath $output)) {
    [void][IO.Directory]::CreateDirectory($output)
}

& cargo build --manifest-path (Join-Path $root 'Cargo.toml') --release --locked --target $Target -p cs2_resedit
if ($LASTEXITCODE -ne 0) { throw 'Build failed.' }

$built = Join-Path $root "target\$Target\release\CS2-ResEdit.exe"
if (-not (Test-Path -LiteralPath $built -PathType Leaf)) { throw 'Built executable was not found.' }
$exe = Join-Path $output 'CS2-ResEdit.exe'
$checksum = "$exe.sha256"
$nonce = [Guid]::NewGuid().ToString('N')
$stagedExe = Join-Path $output "CS2-ResEdit.$nonce.stage.exe"
$stagedChecksum = "$stagedExe.sha256"
$previousExe = Join-Path $output "CS2-ResEdit.$nonce.previous.exe"
$previousChecksum = "$previousExe.sha256"
$hadExe = Test-Path -LiteralPath $exe -PathType Leaf
$hadChecksum = Test-Path -LiteralPath $checksum -PathType Leaf
$cleanupPrevious = $true
try {
    Copy-Item -LiteralPath $built -Destination $stagedExe
    $hash = (Get-FileHash -LiteralPath $stagedExe -Algorithm SHA256).Hash.ToLowerInvariant()
    [IO.File]::WriteAllText($stagedChecksum, "$hash  CS2-ResEdit.exe`n", [Text.UTF8Encoding]::new($false))
    if ((Get-FileHash -LiteralPath $stagedExe -Algorithm SHA256).Hash.ToLowerInvariant() -ne $hash) {
        throw 'Staged executable checksum changed before publication.'
    }

    if ($hadExe) {
        try {
            $handle = [IO.File]::Open($exe, [IO.FileMode]::Open, [IO.FileAccess]::ReadWrite, [IO.FileShare]::None)
            $handle.Dispose()
        } catch {
            throw "The existing CS2-ResEdit.exe is in use. Close it before publishing: $_"
        }
        Copy-Item -LiteralPath $exe -Destination $previousExe
    }
    if ($hadChecksum) { Copy-Item -LiteralPath $checksum -Destination $previousChecksum }

    try {
        Copy-Item -LiteralPath $stagedExe -Destination $exe -Force
        Copy-Item -LiteralPath $stagedChecksum -Destination $checksum -Force
    } catch {
        $promotionError = $_
        try {
            if ($hadExe) { Copy-Item -LiteralPath $previousExe -Destination $exe -Force }
            elseif (Test-Path -LiteralPath $exe) { Remove-Item -LiteralPath $exe -Force }
            if ($hadChecksum) { Copy-Item -LiteralPath $previousChecksum -Destination $checksum -Force }
            elseif (Test-Path -LiteralPath $checksum) { Remove-Item -LiteralPath $checksum -Force }
        } catch {
            $cleanupPrevious = $false
            throw "Publishing failed ($promotionError). Rollback also failed; previous assets are preserved at '$previousExe' and '$previousChecksum': $_"
        }
        throw $promotionError
    }
} finally {
    $cleanupPaths = @($stagedExe, $stagedChecksum)
    if ($cleanupPrevious) { $cleanupPaths += @($previousExe, $previousChecksum) }
    foreach ($path in $cleanupPaths) {
        if (Test-Path -LiteralPath $path) { Remove-Item -LiteralPath $path -Force }
    }
}

[pscustomobject]@{
    Executable = $exe
    Checksum = $checksum
    Sha256 = $hash
    Size = (Get-Item -LiteralPath $exe).Length
}
