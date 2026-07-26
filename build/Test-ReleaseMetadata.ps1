[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [string]$Tag,

    [string]$RefType = 'tag',

    [string]$SourcePath = (Join-Path $PSScriptRoot '..\CS2-VideoConfig-Editor.ps1'),

    [string]$ReleaseNotesPath = (Join-Path $PSScriptRoot '..\RELEASE_NOTES.md')
)

$ErrorActionPreference = 'Stop'
$SourcePath = [IO.Path]::GetFullPath($SourcePath)
$ReleaseNotesPath = [IO.Path]::GetFullPath($ReleaseNotesPath)

. $SourcePath
$expectedTag = "v$script:ApplicationVersion"
if ($RefType -ne 'tag') {
    throw "Release workflow requires a tag ref; received '$RefType'."
}
if ($Tag -ne $expectedTag) {
    throw "Tag '$Tag' does not match source version '$expectedTag'."
}

$releaseHeading = "# CS2 Video Config Editor $expectedTag"
if (-not (Test-Path -LiteralPath $ReleaseNotesPath -PathType Leaf)) {
    throw "Release notes were not found: $ReleaseNotesPath"
}
if ((Get-Content -LiteralPath $ReleaseNotesPath -First 1) -ne $releaseHeading) {
    throw "RELEASE_NOTES.md must begin with '$releaseHeading'."
}

[pscustomobject]@{
    Version = $script:ApplicationVersion
    Tag = $Tag
    ReleaseNotesPath = $ReleaseNotesPath
}
