$ErrorActionPreference = 'Stop'
$script:BackupRetentionCount = 5
$script:VideoConfigFields = [ordered]@{
    Width = 'setting.defaultres'
    Height = 'setting.defaultresheight'
    Mode = 'setting.aspectratiomode'
}
$script:Resolutions = @(
    [pscustomobject]@{ Width = 1024; Height = 768;  Ratio = '4:3';   Mode = '0' }
    [pscustomobject]@{ Width = 1152; Height = 864;  Ratio = '4:3';   Mode = '0' }
    [pscustomobject]@{ Width = 1280; Height = 960;  Ratio = '4:3';   Mode = '0' }
    [pscustomobject]@{ Width = 1440; Height = 1080; Ratio = '4:3';   Mode = '0' }
    [pscustomobject]@{ Width = 1280; Height = 1024; Ratio = '5:4';   Mode = '0' }
    [pscustomobject]@{ Width = 1280; Height = 720;  Ratio = '16:9';  Mode = '1' }
    [pscustomobject]@{ Width = 1600; Height = 900;  Ratio = '16:9';  Mode = '1' }
    [pscustomobject]@{ Width = 1920; Height = 1080; Ratio = '16:9';  Mode = '1' }
    [pscustomobject]@{ Width = 2560; Height = 1440; Ratio = '16:9';  Mode = '1' }
    [pscustomobject]@{ Width = 3840; Height = 2160; Ratio = '16:9';  Mode = '1' }
    [pscustomobject]@{ Width = 1440; Height = 900;  Ratio = '16:10'; Mode = '2' }
    [pscustomobject]@{ Width = 1680; Height = 1050; Ratio = '16:10'; Mode = '2' }
    [pscustomobject]@{ Width = 1728; Height = 1080; Ratio = '16:10'; Mode = '2' }
    [pscustomobject]@{ Width = 1920; Height = 1200; Ratio = '16:10'; Mode = '2' }
)

function Get-Cs2ResolutionPresets {
    @($script:Resolutions | ForEach-Object {
        [pscustomobject]@{
            Width = $_.Width
            Height = $_.Height
            Ratio = $_.Ratio
            Mode = $_.Mode
        }
    })
}

function ConvertTo-AspectMode {
    param([string]$Value)
    switch ($Value) {
        { $_ -in '0', '4:3', '5:4' } { return '0' }
        { $_ -in '1', '16:9' }       { return '1' }
        { $_ -in '2', '16:10' }      { return '2' }
        default                      { return $null }
    }
}

function ConvertFrom-AspectMode {
    param([string]$Value)
    switch ($Value) { '0' { '4:3 / 5:4' } '1' { '16:9' } '2' { '16:10' } default { 'Unknown' } }
}

function Get-AutomaticAspectMode {
    param([int]$Width, [int]$Height)
    $ratio = $Width / [double]$Height
    if ([math]::Abs($ratio - (4 / 3.0)) -lt 0.01 -or [math]::Abs($ratio - (5 / 4.0)) -lt 0.01) { return '0' }
    if ([math]::Abs($ratio - (16 / 9.0)) -lt 0.01) { return '1' }
    if ([math]::Abs($ratio - (16 / 10.0)) -lt 0.01) { return '2' }
    return $null
}

function New-Resolution {
    param([int]$Width, [int]$Height, [AllowNull()][string]$Mode)
    $normalizedMode = if ([string]::IsNullOrEmpty($Mode)) { $null } else { $Mode }
    [pscustomobject]@{ Width = $Width; Height = $Height; Mode = $normalizedMode; Display = "${Width}x${Height}" }
}

function Resolve-Resolution {
    param([string]$Value)
    if ([string]::IsNullOrWhiteSpace($Value)) { return $null }
    $Value = $Value.Trim()
    if ($Value -match '^\d+$') {
        $index = [int]$Value
        if ($index -ge 1 -and $index -le $script:Resolutions.Count) {
            $item = $script:Resolutions[$index - 1]
            return New-Resolution $item.Width $item.Height $item.Mode
        }
        return $null
    }
    if ($Value -notmatch '^(?<width>\d+)\s*[xX]\s*(?<height>\d+)$') { return $null }
    $width = [int]$Matches.width
    $height = [int]$Matches.height
    if ($width -lt 320 -or $height -lt 200 -or $width -gt 32768 -or $height -gt 32768) { return $null }
    $known = $script:Resolutions | Where-Object { $_.Width -eq $width -and $_.Height -eq $height } | Select-Object -First 1
    $mode = if ($null -ne $known) { $known.Mode } else { Get-AutomaticAspectMode $width $height }
    New-Resolution $width $height $mode
}

function Resolve-SelectedResolution {
    param(
        [string]$Selection,
        [int]$CustomWidth,
        [int]$CustomHeight,
        [string]$Mode
    )
    if ($Selection -eq 'Custom...') {
        return New-Resolution $CustomWidth $CustomHeight $Mode
    }

    $resolution = Resolve-Resolution $Selection
    if ($null -eq $resolution) { throw "Invalid resolution selection '$Selection'." }
    $resolution.Mode = $Mode
    $resolution
}

function Get-TextFileInfo {
    param([string]$Path)
    $bytes = [IO.File]::ReadAllBytes($Path)
    $encoding = New-Object Text.UTF8Encoding($false, $true)
    $offset = 0
    if ($bytes.Length -ge 3 -and $bytes[0] -eq 0xEF -and $bytes[1] -eq 0xBB -and $bytes[2] -eq 0xBF) {
        $encoding = New-Object Text.UTF8Encoding($true); $offset = 3
    } elseif ($bytes.Length -ge 2 -and $bytes[0] -eq 0xFF -and $bytes[1] -eq 0xFE) {
        $encoding = New-Object Text.UnicodeEncoding($false, $true); $offset = 2
    } elseif ($bytes.Length -ge 2 -and $bytes[0] -eq 0xFE -and $bytes[1] -eq 0xFF) {
        $encoding = New-Object Text.UnicodeEncoding($true, $true); $offset = 2
    }
    try {
        $text = $encoding.GetString($bytes, $offset, $bytes.Length - $offset)
    } catch [Text.DecoderFallbackException] {
        # Old Steam files may use the current Windows ANSI code page.
        $encoding = [Text.Encoding]::Default
        $text = $encoding.GetString($bytes, $offset, $bytes.Length - $offset)
    }
    [pscustomobject]@{
        Text = $text
        Encoding = $encoding
    }
}

function Get-ConfigValue {
    param([string]$Text, [string]$Key)
    $pattern = '(?m)^[\t ]*"' + [regex]::Escape($Key) + '"[\t ]*"(?<value>[^"\r\n]*)"[\t ]*\r?$'
    $matches = [regex]::Matches($Text, $pattern)
    if ($matches.Count -ne 1) { throw "Expected exactly one '$Key' entry; found $($matches.Count)." }
    $matches[0].Groups['value'].Value
}

function Get-CurrentConfig {
    param([string]$Path)
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) { throw "Configuration file not found: $Path" }
    $file = Get-TextFileInfo $Path
    [pscustomobject]@{
        Width = [int](Get-ConfigValue $file.Text $script:VideoConfigFields.Width)
        Height = [int](Get-ConfigValue $file.Text $script:VideoConfigFields.Height)
        Mode = Get-ConfigValue $file.Text $script:VideoConfigFields.Mode
    }
}

function Set-ConfigValue {
    param([string]$Text, [string]$Key, [string]$Value)
    $pattern = '(?m)^(?<prefix>[\t ]*"' + [regex]::Escape($Key) + '"[\t ]*)"[^"\r\n]*"(?<suffix>[\t ]*)\r?$'
    $matches = [regex]::Matches($Text, $pattern)
    if ($matches.Count -ne 1) { throw "Expected exactly one '$Key' entry; found $($matches.Count). No file was written." }
    [regex]::Replace(
        $Text,
        $pattern,
        { param($match) $match.Groups['prefix'].Value + '"' + $Value + '"' + $match.Groups['suffix'].Value + $(if ($match.Value.EndsWith("`r")) { "`r" } else { '' }) },
        1
    )
}

function New-Cs2BackupPath {
    param([string]$Path)
    $stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
    $candidate = "$Path.$stamp.bak"
    $suffix = 1
    while (Test-Path -LiteralPath $candidate) {
        $candidate = "$Path.$stamp-$suffix.bak"
        $suffix++
    }
    $candidate
}

function Get-Cs2BackupFiles {
    param([string]$Path)
    $fullPath = [IO.Path]::GetFullPath($Path)
    $directory = [IO.Path]::GetDirectoryName($fullPath)
    if (-not (Test-Path -LiteralPath $directory -PathType Container)) { return @() }
    $fileName = [IO.Path]::GetFileName($fullPath)
    $pattern = '^' + [regex]::Escape($fileName) + '\.(?<stamp>\d{8}-\d{6})(?:-(?<suffix>\d+))?\.bak$'
    $backups = foreach ($file in Get-ChildItem -LiteralPath $directory -File -ErrorAction SilentlyContinue) {
        $match = [regex]::Match($file.Name, $pattern)
        if (-not $match.Success) { continue }
        $stamp = [datetime]::MinValue
        [void][datetime]::TryParseExact(
            $match.Groups['stamp'].Value,
            'yyyyMMdd-HHmmss',
            [Globalization.CultureInfo]::InvariantCulture,
            [Globalization.DateTimeStyles]::None,
            [ref]$stamp
        )
        $config = $null
        $errorText = $null
        try { $config = Get-CurrentConfig $file.FullName } catch { $errorText = $_.Exception.Message }
        [pscustomobject]@{
            Path = $file.FullName
            Name = $file.Name
            Timestamp = $stamp
            Suffix = if ($match.Groups['suffix'].Success) { [int]$match.Groups['suffix'].Value } else { 0 }
            LastWriteTime = $file.LastWriteTime
            Config = $config
            Valid = $null -ne $config
            Error = $errorText
        }
    }
    @($backups | Sort-Object @{ Expression = 'Timestamp'; Descending = $true }, @{ Expression = 'Suffix'; Descending = $true }, @{ Expression = 'LastWriteTime'; Descending = $true })
}

function Remove-Cs2OldBackups {
    param([string]$Path, [int]$Keep = $script:BackupRetentionCount)
    if ($Keep -lt 1) { throw 'Backup retention must keep at least one file.' }
    $removed = New-Object System.Collections.Generic.List[string]
    $backups = @(Get-Cs2BackupFiles $Path)
    foreach ($backup in $backups | Select-Object -Skip $Keep) {
        Remove-Item -LiteralPath $backup.Path -Force
        [void]$removed.Add($backup.Path)
    }
    @($removed)
}

function Invoke-Cs2BackupRetention {
    param([string]$Path, [int]$Keep = $script:BackupRetentionCount)
    try {
        [pscustomobject]@{
            RemovedBackups = @(Remove-Cs2OldBackups $Path $Keep)
            Warning = $null
        }
    } catch {
        [pscustomobject]@{
            RemovedBackups = @()
            Warning = "Backup retention could not be completed: $($_.Exception.Message)"
        }
    }
}

function Test-Cs2ConfigEquals {
    param($Left, $Right)
    $null -ne $Left -and $null -ne $Right -and
        $Left.Width -eq $Right.Width -and
        $Left.Height -eq $Right.Height -and
        [string]$Left.Mode -eq [string]$Right.Mode
}

function Restore-Cs2Backup {
    param([string]$Path, [string]$BackupPath)
    $fullPath = [IO.Path]::GetFullPath($Path)
    $fullBackupPath = [IO.Path]::GetFullPath($BackupPath)
    $knownBackup = @(Get-Cs2BackupFiles $fullPath | Where-Object Path -eq $fullBackupPath | Select-Object -First 1)
    if ($knownBackup.Count -ne 1) { throw 'The selected file is not an editor-created backup for this configuration.' }
    if (-not $knownBackup[0].Valid) { throw "The selected backup is invalid: $($knownBackup[0].Error)" }
    $current = Get-CurrentConfig $fullPath
    if (Test-Cs2ConfigEquals $current $knownBackup[0].Config) {
        return [pscustomobject]@{ Changed = $false; BackupPath = $null; RestoredConfig = $current; RemovedBackups = @(); RetentionWarning = $null }
    }
    $rollbackPath = New-Cs2BackupPath $fullPath
    $tempPath = Join-Path ([IO.Path]::GetDirectoryName($fullPath)) ('.cs2-video-' + [guid]::NewGuid().ToString('N') + '.tmp')
    try {
        [IO.File]::Copy($fullBackupPath, $tempPath, $false)
        [void](Get-CurrentConfig $tempPath)
        [IO.File]::Replace($tempPath, $fullPath, $rollbackPath)
    } finally {
        if (Test-Path -LiteralPath $tempPath) { Remove-Item -LiteralPath $tempPath -Force -ErrorAction SilentlyContinue }
    }
    $restored = Get-CurrentConfig $fullPath
    if (-not (Test-Cs2ConfigEquals $restored $knownBackup[0].Config)) {
        throw 'The restored configuration did not match the selected backup.'
    }
    $retention = Invoke-Cs2BackupRetention $fullPath
    [pscustomobject]@{
        Changed = $true
        BackupPath = $rollbackPath
        RestoredConfig = $restored
        RemovedBackups = @($retention.RemovedBackups)
        RetentionWarning = $retention.Warning
    }
}

function Update-VideoConfig {
    param(
        [string]$Path,
        [psobject]$Resolution,
        [bool]$CreateBackup = $true
    )
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) { throw "Configuration file not found: $Path" }
    $file = Get-TextFileInfo $Path

    # Validate and update every required entry in memory before touching the file.
    $updated = $file.Text
    foreach ($property in $script:VideoConfigFields.Keys) {
        $updated = Set-ConfigValue $updated $script:VideoConfigFields[$property] ([string]$Resolution.$property)
    }
    if ($updated -ceq $file.Text) {
        return [pscustomobject]@{ Changed = $false; BackupPath = $null; RemovedBackups = @(); RetentionWarning = $null }
    }

    $replaceBackupPath = New-Cs2BackupPath $Path
    $backupPath = if ($CreateBackup) { $replaceBackupPath } else { $null }

    $tempPath = Join-Path ([IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($Path))) ('.cs2-video-' + [guid]::NewGuid().ToString('N') + '.tmp')
    try {
        [IO.File]::WriteAllText($tempPath, $updated, $file.Encoding)
        # File.Replace requires a backup path on Windows PowerShell 5.1. Create
        # one for the atomic swap and remove it when the user opted out.
        [IO.File]::Replace($tempPath, [IO.Path]::GetFullPath($Path), $replaceBackupPath)
        if (-not $CreateBackup) { Remove-Item -LiteralPath $replaceBackupPath -Force }
    } finally {
        if (Test-Path -LiteralPath $tempPath) { Remove-Item -LiteralPath $tempPath -Force -ErrorAction SilentlyContinue }
    }
    $retention = if ($CreateBackup) {
        Invoke-Cs2BackupRetention $Path
    } else {
        [pscustomobject]@{ RemovedBackups = @(); Warning = $null }
    }
    [pscustomobject]@{
        Changed = $true
        BackupPath = $backupPath
        RemovedBackups = @($retention.RemovedBackups)
        RetentionWarning = $retention.Warning
    }
}

Export-ModuleMember -Function @(
    'Get-Cs2ResolutionPresets',
    'ConvertTo-AspectMode',
    'ConvertFrom-AspectMode',
    'Get-AutomaticAspectMode',
    'New-Resolution',
    'Resolve-Resolution',
    'Resolve-SelectedResolution',
    'Get-TextFileInfo',
    'Get-ConfigValue',
    'Get-CurrentConfig',
    'Set-ConfigValue',
    'New-Cs2BackupPath',
    'Get-Cs2BackupFiles',
    'Remove-Cs2OldBackups',
    'Invoke-Cs2BackupRetention',
    'Test-Cs2ConfigEquals',
    'Restore-Cs2Backup',
    'Update-VideoConfig'
)
