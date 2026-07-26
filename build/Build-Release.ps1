[CmdletBinding()]
param(
    [string]$Version = '1.2.0',
    [string]$OutputDirectory
)

$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$sourcePath = Join-Path $root 'CS2-VideoConfig-Editor.ps1'
if ([string]::IsNullOrWhiteSpace($OutputDirectory)) { $OutputDirectory = $root }
$OutputDirectory = [IO.Path]::GetFullPath($OutputDirectory)
[void][IO.Directory]::CreateDirectory($OutputDirectory)

. $sourcePath
if ($script:ApplicationVersion -ne $Version) {
    throw "Requested version '$Version' does not match source version '$script:ApplicationVersion'."
}

Add-Type -AssemblyName System.IO.Compression
Add-Type -AssemblyName System.IO.Compression.FileSystem
$zipPath = Join-Path $OutputDirectory 'CS2-VideoConfig-Editor.zip'
$checksumPath = Join-Path $OutputDirectory 'CS2-VideoConfig-Editor.zip.sha256'
if (Test-Path -LiteralPath $zipPath) { Remove-Item -LiteralPath $zipPath -Force }

$fileStream = [IO.File]::Open($zipPath, [IO.FileMode]::CreateNew, [IO.FileAccess]::ReadWrite, [IO.FileShare]::None)
try {
    $archive = New-Object IO.Compression.ZipArchive($fileStream, [IO.Compression.ZipArchiveMode]::Create, $false)
    try {
        $entry = $archive.CreateEntry('CS2-VideoConfig-Editor.ps1', [IO.Compression.CompressionLevel]::NoCompression)
        $entry.LastWriteTime = New-Object DateTimeOffset(1980, 1, 1, 0, 0, 0, [TimeSpan]::Zero)
        $entryStream = $entry.Open()
        try {
            $sourceStream = [IO.File]::OpenRead($sourcePath)
            try { $sourceStream.CopyTo($entryStream) } finally { $sourceStream.Dispose() }
        } finally { $entryStream.Dispose() }
    } finally { $archive.Dispose() }
} finally { $fileStream.Dispose() }

$sourceHash = (Get-FileHash $sourcePath -Algorithm SHA256).Hash
$archive = [IO.Compression.ZipFile]::OpenRead($zipPath)
try {
    if ($archive.Entries.Count -ne 1) { throw "Expected one ZIP entry; found $($archive.Entries.Count)." }
    $entry = $archive.Entries[0]
    if ($entry.FullName -ne 'CS2-VideoConfig-Editor.ps1') { throw "Unexpected ZIP entry '$($entry.FullName)'." }
    $sha = [Security.Cryptography.SHA256]::Create()
    try {
        $entryStream = $entry.Open()
        try { $entryHash = ([BitConverter]::ToString($sha.ComputeHash($entryStream))).Replace('-', '') }
        finally { $entryStream.Dispose() }
    } finally { $sha.Dispose() }
} finally { $archive.Dispose() }
if ($entryHash -ne $sourceHash) { throw 'The packaged source does not match the repository source.' }

$zipHash = (Get-FileHash $zipPath -Algorithm SHA256).Hash.ToLowerInvariant()
[IO.File]::WriteAllText($checksumPath, "$zipHash  CS2-VideoConfig-Editor.zip`r`n", (New-Object Text.ASCIIEncoding))

[pscustomobject]@{
    Version = $Version
    SourcePath = $sourcePath
    SourceSHA256 = $sourceHash
    ZipPath = $zipPath
    ZipSHA256 = $zipHash
    ChecksumPath = $checksumPath
}
