[CmdletBinding()]
param(
    [string]$Version = '2.0.0',
    [string]$OutputDirectory,
    [string]$ManifestPath
)

$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$sourcePath = Join-Path $root 'CS2-VideoConfig-Editor.ps1'
if ([string]::IsNullOrWhiteSpace($OutputDirectory)) { $OutputDirectory = $root }
if ([string]::IsNullOrWhiteSpace($ManifestPath)) {
    $ManifestPath = Join-Path $PSScriptRoot 'ReleaseFiles.psd1'
}
$OutputDirectory = [IO.Path]::GetFullPath($OutputDirectory)
$ManifestPath = [IO.Path]::GetFullPath($ManifestPath)
[void][IO.Directory]::CreateDirectory($OutputDirectory)

. $sourcePath
if ($script:ApplicationVersion -ne $Version) {
    throw "Requested version '$Version' does not match source version '$script:ApplicationVersion'."
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

$rootPrefix = $root.TrimEnd([IO.Path]::DirectorySeparatorChar, [IO.Path]::AltDirectorySeparatorChar) +
    [IO.Path]::DirectorySeparatorChar
$seenPaths = @{}
$releaseFiles = foreach ($relativePath in $manifestFiles) {
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

    $fullPath = [IO.Path]::GetFullPath((Join-Path $root ($archivePath.Replace('/', [IO.Path]::DirectorySeparatorChar))))
    if (-not $fullPath.StartsWith($rootPrefix, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Release manifest path escapes the repository root: '$relativePath'."
    }
    if (-not (Test-Path -LiteralPath $fullPath -PathType Leaf)) {
        throw "Release file was not found: $fullPath"
    }

    [pscustomobject]@{
        ArchivePath = $archivePath
        FullPath = $fullPath
    }
}
$releaseFiles = @($releaseFiles)

function Get-Crc32 {
    param([Parameter(Mandatory)][byte[]]$Bytes)

    [uint32]$crc = [uint32]::MaxValue
    foreach ($value in $Bytes) {
        $crc = [uint32]($crc -bxor [uint32]$value)
        for ($bit = 0; $bit -lt 8; $bit++) {
            if (($crc -band [uint32]1) -ne 0) {
                $crc = [uint32]([uint32]3988292384 -bxor ($crc -shr 1))
            } else {
                $crc = [uint32]($crc -shr 1)
            }
        }
    }
    return [uint32]($crc -bxor [uint32]::MaxValue)
}

function Write-UInt16 {
    param(
        [Parameter(Mandatory)][IO.BinaryWriter]$Writer,
        [Parameter(Mandatory)][uint16]$Value
    )
    $Writer.Write($Value)
}

function Write-UInt32 {
    param(
        [Parameter(Mandatory)][IO.BinaryWriter]$Writer,
        [Parameter(Mandatory)][uint32]$Value
    )
    $Writer.Write($Value)
}

# ZipArchive writes different bytes on .NET Framework and modern .NET for
# CompressionLevel.NoCompression. Write a minimal ZIP32 archive explicitly so
# Windows PowerShell 5.1 and PowerShell 7 produce the same release artifact.
$zipPath = Join-Path $OutputDirectory 'CS2-VideoConfig-Editor.zip'
$checksumPath = Join-Path $OutputDirectory 'CS2-VideoConfig-Editor.zip.sha256'
$temporaryZipPath = "$zipPath.tmp"
if (Test-Path -LiteralPath $temporaryZipPath) { Remove-Item -LiteralPath $temporaryZipPath -Force }

$utf8 = New-Object Text.UTF8Encoding($false)
$utf8Flag = [uint16]0x0800
$dosTime = [uint16]0
$dosDate = [uint16]33 # 1980-01-01
$records = @()

try {
    $fileStream = [IO.File]::Open(
        $temporaryZipPath,
        [IO.FileMode]::CreateNew,
        [IO.FileAccess]::Write,
        [IO.FileShare]::None
    )
    try {
        $writer = New-Object IO.BinaryWriter($fileStream, $utf8, $true)
        try {
            foreach ($releaseFile in $releaseFiles) {
                [byte[]]$nameBytes = $utf8.GetBytes($releaseFile.ArchivePath)
                [byte[]]$contentBytes = [IO.File]::ReadAllBytes($releaseFile.FullPath)
                if ($nameBytes.Length -gt [uint16]::MaxValue) {
                    throw "Archive path is too long for ZIP32: '$($releaseFile.ArchivePath)'."
                }
                if ([uint64]$contentBytes.LongLength -gt [uint32]::MaxValue) {
                    throw "Release file is too large for ZIP32: '$($releaseFile.FullPath)'."
                }
                if ([uint64]$fileStream.Position -gt [uint32]::MaxValue) {
                    throw 'Release archive is too large for ZIP32.'
                }

                [uint32]$offset = $fileStream.Position
                [uint32]$length = $contentBytes.Length
                [uint32]$crc32 = Get-Crc32 -Bytes $contentBytes

                Write-UInt32 $writer 0x04034b50
                Write-UInt16 $writer 20
                Write-UInt16 $writer $utf8Flag
                Write-UInt16 $writer 0
                Write-UInt16 $writer $dosTime
                Write-UInt16 $writer $dosDate
                Write-UInt32 $writer $crc32
                Write-UInt32 $writer $length
                Write-UInt32 $writer $length
                Write-UInt16 $writer $nameBytes.Length
                Write-UInt16 $writer 0
                $writer.Write($nameBytes)
                $writer.Write($contentBytes)

                $records += [pscustomobject]@{
                    NameBytes = $nameBytes
                    Crc32 = $crc32
                    Length = $length
                    Offset = $offset
                }
            }

            if ([uint64]$fileStream.Position -gt [uint32]::MaxValue) {
                throw 'Release archive is too large for ZIP32.'
            }
            [uint32]$centralDirectoryOffset = $fileStream.Position
            foreach ($record in $records) {
                Write-UInt32 $writer 0x02014b50
                Write-UInt16 $writer 20
                Write-UInt16 $writer 20
                Write-UInt16 $writer $utf8Flag
                Write-UInt16 $writer 0
                Write-UInt16 $writer $dosTime
                Write-UInt16 $writer $dosDate
                Write-UInt32 $writer $record.Crc32
                Write-UInt32 $writer $record.Length
                Write-UInt32 $writer $record.Length
                Write-UInt16 $writer $record.NameBytes.Length
                Write-UInt16 $writer 0
                Write-UInt16 $writer 0
                Write-UInt16 $writer 0
                Write-UInt16 $writer 0
                Write-UInt32 $writer 0
                Write-UInt32 $writer $record.Offset
                $writer.Write([byte[]]$record.NameBytes)
            }

            [uint64]$centralDirectoryLength64 = $fileStream.Position - $centralDirectoryOffset
            if ($centralDirectoryLength64 -gt [uint32]::MaxValue) {
                throw 'Release archive central directory is too large for ZIP32.'
            }
            if ($records.Count -gt [uint16]::MaxValue) {
                throw 'Release archive has too many entries for ZIP32.'
            }
            [uint32]$centralDirectoryLength = $centralDirectoryLength64
            [uint16]$entryCount = $records.Count

            Write-UInt32 $writer 0x06054b50
            Write-UInt16 $writer 0
            Write-UInt16 $writer 0
            Write-UInt16 $writer $entryCount
            Write-UInt16 $writer $entryCount
            Write-UInt32 $writer $centralDirectoryLength
            Write-UInt32 $writer $centralDirectoryOffset
            Write-UInt16 $writer 0
            $writer.Flush()
        } finally {
            $writer.Dispose()
        }
    } finally {
        $fileStream.Dispose()
    }

    if (Test-Path -LiteralPath $zipPath) { Remove-Item -LiteralPath $zipPath -Force }
    Move-Item -LiteralPath $temporaryZipPath -Destination $zipPath
} finally {
    if (Test-Path -LiteralPath $temporaryZipPath) {
        Remove-Item -LiteralPath $temporaryZipPath -Force
    }
}

$zipHash = (Get-FileHash $zipPath -Algorithm SHA256).Hash.ToLowerInvariant()
[IO.File]::WriteAllText(
    $checksumPath,
    "$zipHash  CS2-VideoConfig-Editor.zip`r`n",
    (New-Object Text.ASCIIEncoding)
)

$verification = & (Join-Path $PSScriptRoot 'Test-ReleaseArtifacts.ps1') `
    -ArtifactDirectory $OutputDirectory `
    -SourceRoot $root `
    -ManifestPath $ManifestPath

[pscustomobject]@{
    Version = $Version
    SourcePath = $sourcePath
    SourceSHA256 = (Get-FileHash $sourcePath -Algorithm SHA256).Hash
    FileCount = $verification.FileCount
    ZipPath = $zipPath
    ZipSHA256 = $zipHash
    ChecksumPath = $checksumPath
}
