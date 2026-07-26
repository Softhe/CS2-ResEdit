[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [string]$ArtifactDirectory,

    [string]$SourceRoot,

    [string]$ManifestPath,

    [switch]$RequireAuthenticodeSignature
)

$ErrorActionPreference = 'Stop'
if ([string]::IsNullOrWhiteSpace($SourceRoot)) {
    $SourceRoot = Join-Path $PSScriptRoot '..'
}
if ([string]::IsNullOrWhiteSpace($ManifestPath)) {
    $ManifestPath = Join-Path $PSScriptRoot 'ReleaseFiles.psd1'
}
$ArtifactDirectory = [IO.Path]::GetFullPath($ArtifactDirectory)
$SourceRoot = [IO.Path]::GetFullPath($SourceRoot)
$ManifestPath = [IO.Path]::GetFullPath($ManifestPath)
$zipPath = Join-Path $ArtifactDirectory 'CS2-VideoConfig-Editor.zip'
$checksumPath = Join-Path $ArtifactDirectory 'CS2-VideoConfig-Editor.zip.sha256'

if (-not (Test-Path -LiteralPath $zipPath -PathType Leaf)) {
    throw "Release ZIP was not found: $zipPath"
}
if (-not (Test-Path -LiteralPath $checksumPath -PathType Leaf)) {
    throw "Release checksum was not found: $checksumPath"
}
if (-not (Test-Path -LiteralPath $ManifestPath -PathType Leaf)) {
    throw "Release file manifest was not found: $ManifestPath"
}

$manifest = Import-PowerShellDataFile -LiteralPath $ManifestPath
if ($manifest.FormatVersion -ne 1) {
    throw "Unsupported release manifest format '$($manifest.FormatVersion)'."
}
$manifestFiles = @($manifest.Files)
if ($manifestFiles.Count -eq 0) {
    throw 'Release file manifest must contain at least one file.'
}

$sourceRootPrefix = $SourceRoot.TrimEnd(
    [IO.Path]::DirectorySeparatorChar,
    [IO.Path]::AltDirectorySeparatorChar
) + [IO.Path]::DirectorySeparatorChar
$seenPaths = @{}
$expectedFiles = foreach ($relativePath in $manifestFiles) {
    if (-not ($relativePath -is [string]) -or [string]::IsNullOrWhiteSpace($relativePath)) {
        throw 'Every release manifest file must be a non-empty string.'
    }

    $archivePath = $relativePath.Replace('\', '/')
    $segments = @($archivePath.Split('/'))
    if (
        [IO.Path]::IsPathRooted($relativePath) -or
        $segments.Count -eq 0 -or
        @($segments | Where-Object { $_ -eq '' -or $_ -eq '.' -or $_ -eq '..' }).Count -gt 0
    ) {
        throw "Release manifest path must be a normalized relative path: '$relativePath'."
    }
    if ($seenPaths.ContainsKey($archivePath)) {
        throw "Release manifest contains a duplicate or case-colliding path: '$archivePath'."
    }
    $seenPaths[$archivePath] = $true

    $fullPath = [IO.Path]::GetFullPath(
        (Join-Path $SourceRoot ($archivePath.Replace('/', [IO.Path]::DirectorySeparatorChar)))
    )
    if (-not $fullPath.StartsWith($sourceRootPrefix, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Release manifest path escapes the source root: '$relativePath'."
    }
    if (-not (Test-Path -LiteralPath $fullPath -PathType Leaf)) {
        throw "Expected release source file was not found: $fullPath"
    }

    [pscustomobject]@{
        ArchivePath = $archivePath
        FullPath = $fullPath
    }
}
$expectedFiles = @($expectedFiles)

$zipHash = (Get-FileHash -LiteralPath $zipPath -Algorithm SHA256).Hash.ToLowerInvariant()
$checksumText = [IO.File]::ReadAllText($checksumPath, [Text.Encoding]::ASCII)
$expectedChecksumText = "$zipHash  CS2-VideoConfig-Editor.zip`r`n"
if ($checksumText -ne $expectedChecksumText) {
    throw 'Release checksum sidecar does not exactly match the ZIP SHA-256.'
}

Add-Type -AssemblyName System.IO.Compression
Add-Type -AssemblyName System.IO.Compression.FileSystem
$archive = [IO.Compression.ZipFile]::OpenRead($zipPath)
try {
    if ($archive.Entries.Count -ne $expectedFiles.Count) {
        throw "Expected $($expectedFiles.Count) ZIP entries; found $($archive.Entries.Count)."
    }

    $entries = @{}
    foreach ($entry in $archive.Entries) {
        if ($entries.ContainsKey($entry.FullName)) {
            throw "Release ZIP contains a duplicate or case-colliding entry: '$($entry.FullName)'."
        }
        $entries[$entry.FullName] = $entry
    }

    foreach ($expectedFile in $expectedFiles) {
        if (-not $entries.ContainsKey($expectedFile.ArchivePath)) {
            throw "Release ZIP is missing '$($expectedFile.ArchivePath)'."
        }
        $entry = $entries[$expectedFile.ArchivePath]
        [byte[]]$sourceBytes = [IO.File]::ReadAllBytes($expectedFile.FullPath)
        if ($entry.Length -ne $sourceBytes.LongLength) {
            throw "Packaged file length differs from source: '$($expectedFile.ArchivePath)'."
        }

        $memoryStream = New-Object IO.MemoryStream
        try {
            $entryStream = $entry.Open()
            try {
                $entryStream.CopyTo($memoryStream)
            } finally {
                $entryStream.Dispose()
            }
            [byte[]]$packagedBytes = $memoryStream.ToArray()
        } finally {
            $memoryStream.Dispose()
        }

        for ($index = 0; $index -lt $sourceBytes.Length; $index++) {
            if ($sourceBytes[$index] -ne $packagedBytes[$index]) {
                throw "Packaged file bytes differ from source: '$($expectedFile.ArchivePath)'."
            }
        }
    }
} finally {
    $archive.Dispose()
}

$signedFileCount = 0
if ($RequireAuthenticodeSignature) {
    $authenticodeCommand = Get-Command Get-AuthenticodeSignature -ErrorAction SilentlyContinue
    if ($null -eq $authenticodeCommand) {
        throw 'Authenticode verification is unavailable on this platform.'
    }

    $signableFiles = @(
        $expectedFiles |
            Where-Object { [IO.Path]::GetExtension($_.ArchivePath) -in @('.ps1', '.psm1', '.psd1') }
    )
    if ($signableFiles.Count -eq 0) {
        throw 'Release manifest does not contain any Authenticode-signable files.'
    }
    foreach ($signableFile in $signableFiles) {
        $signature = Get-AuthenticodeSignature -LiteralPath $signableFile.FullPath
        if ($signature.Status -ne [Management.Automation.SignatureStatus]::Valid) {
            throw "Authenticode signature is not valid for '$($signableFile.ArchivePath)': $($signature.StatusMessage)"
        }
    }
    $signedFileCount = $signableFiles.Count
}

[pscustomobject]@{
    ArtifactDirectory = $ArtifactDirectory
    ZipPath = $zipPath
    ZipSHA256 = $zipHash
    FileCount = $expectedFiles.Count
    SignatureRequired = [bool]$RequireAuthenticodeSignature
    SignedFileCount = $signedFileCount
}
