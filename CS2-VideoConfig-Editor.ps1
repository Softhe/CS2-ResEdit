<#
.SYNOPSIS
    Safely edits Counter-Strike 2 resolution settings.
.DESCRIPTION
    Opens a graphical editor when run without -Preset. Steam configuration files
    are discovered automatically, current values are shown before applying, and
    a timestamped backup is created by default.
.PARAMETER FilePath
    Explicit path to cs2_video.txt. When omitted, the most recently modified CS2
    video configuration beneath Steam\userdata is selected automatically.
.PARAMETER SteamRoot
    Optional Steam installation folder override, useful for nonstandard installs.
.PARAMETER Preset
    A resolution such as 1920x1080, or a one-based predefined-resolution number.
.PARAMETER AspectRatioMode
    Optional aspect override: 0/4:3, 1/16:9, or 2/16:10.
.PARAMETER Console
    Use the interactive console interface instead of the graphical interface.
.PARAMETER NoBackup
    Do not create a timestamped .bak file before changing the configuration.
.PARAMETER Silent
    Suppress informational output. Requires -Preset.
.PARAMETER ListAccounts
    Lists locally discovered Steam accounts and their CS2 configuration paths.
.EXAMPLE
    .\CS2-VideoConfig-Editor.ps1
.EXAMPLE
    .\CS2-VideoConfig-Editor.ps1 -Console
.EXAMPLE
    .\CS2-VideoConfig-Editor.ps1 -Preset 1920x1080 -Silent
#>
[CmdletBinding()]
param(
    [Parameter()]
    [ValidateNotNullOrEmpty()]
    [string]$FilePath,

    [Parameter()]
    [ValidateNotNullOrEmpty()]
    [string]$SteamRoot,

    [Parameter()]
    [string]$Preset,

    [Parameter()]
    [ValidateSet('0', '1', '2', '4:3', '16:9', '16:10')]
    [string]$AspectRatioMode,

    [Parameter()]
    [switch]$Console,

    [Parameter()]
    [switch]$NoBackup,

    [Parameter()]
    [switch]$Silent,

    [Parameter()]
    [switch]$ListAccounts
)

$ErrorActionPreference = 'Stop'
$script:SelectedConfigPath = $null

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
    [pscustomobject]@{ Width = 1920; Height = 1200; Ratio = '16:10'; Mode = '2' }
)

function Write-Info {
    param([string]$Message, [ConsoleColor]$Color = 'Gray')
    if (-not $Silent) { Write-Host $Message -ForegroundColor $Color }
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

function Get-SteamRoots {
    if ($SteamRoot) {
        if (-not (Test-Path -LiteralPath $SteamRoot -PathType Container)) { throw "Steam folder not found: $SteamRoot" }
        return [IO.Path]::GetFullPath($SteamRoot)
    }
    $candidates = New-Object System.Collections.Generic.List[string]
    foreach ($registryPath in @('HKCU:\Software\Valve\Steam', 'HKLM:\SOFTWARE\WOW6432Node\Valve\Steam')) {
        try {
            $properties = Get-ItemProperty -LiteralPath $registryPath -ErrorAction Stop
            foreach ($name in @('SteamPath', 'InstallPath')) {
                if ($properties.$name) { [void]$candidates.Add([string]$properties.$name) }
            }
        } catch { }
    }
    if (${env:ProgramFiles(x86)}) { [void]$candidates.Add((Join-Path ${env:ProgramFiles(x86)} 'Steam')) }
    $candidates | Where-Object { $_ -and (Test-Path -LiteralPath $_ -PathType Container) } | Select-Object -Unique
}

function Get-VdfField {
    param([string]$Text, [string]$Name)
    $match = [regex]::Match($Text, '(?m)^[\t ]*"' + [regex]::Escape($Name) + '"[\t ]*"(?<value>(?:\\.|[^"\\])*)"[\t ]*$')
    if ($match.Success) { return $match.Groups['value'].Value -replace '\\"', '"' -replace '\\\\', '\' }
    return $null
}

function Get-SteamLoginUsers {
    param([string]$SteamRoot)
    $path = Join-Path $SteamRoot 'config\loginusers.vdf'
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { return @() }
    $text = Get-Content -LiteralPath $path -Raw
    $pattern = '(?ms)^[\t ]*"(?<steamId>\d{17})"[\t ]*\r?\n[\t ]*\{(?<body>.*?)^[\t ]*\}'
    foreach ($match in [regex]::Matches($text, $pattern)) {
        $body = $match.Groups['body'].Value
        [pscustomobject]@{
            SteamId64 = $match.Groups['steamId'].Value
            AccountName = Get-VdfField $body 'AccountName'
            PersonaName = Get-VdfField $body 'PersonaName'
            MostRecent = (Get-VdfField $body 'MostRecent') -eq '1'
        }
    }
}

function Get-SteamAccounts {
    $steamId64Base = [uint64]76561197960265728
    $accounts = foreach ($root in Get-SteamRoots) {
        $loginUsers = @{}
        foreach ($user in Get-SteamLoginUsers $root) { $loginUsers[$user.SteamId64] = $user }
        $userdata = Join-Path $root 'userdata'
        if (-not (Test-Path -LiteralPath $userdata -PathType Container)) { continue }

        foreach ($directory in Get-ChildItem -LiteralPath $userdata -Directory -ErrorAction SilentlyContinue) {
            $accountId = 0L
            if (-not [long]::TryParse($directory.Name, [ref]$accountId) -or $accountId -lt 0) { continue }
            $steamId64 = ([uint64]$accountId + $steamId64Base).ToString()
            $login = $loginUsers[$steamId64]
            $configPath = Join-Path $directory.FullName '730\local\cfg\cs2_video.txt'
            $hasConfig = Test-Path -LiteralPath $configPath -PathType Leaf
            $personaName = if ($login -and $login.PersonaName) { $login.PersonaName } else { "Steam account $accountId" }
            $displayName = "$personaName  -  Account ID $accountId  -  SteamID64 $steamId64"
            if (-not $hasConfig) { $displayName += '  (CS2 config not found)' }
            [pscustomobject]@{
                DisplayName = $displayName
                AccountId = [string]$accountId
                SteamId64 = $steamId64
                PersonaName = $personaName
                AccountName = if ($login) { $login.AccountName } else { $null }
                MostRecent = [bool]($login -and $login.MostRecent)
                ConfigPath = $configPath
                HasConfig = $hasConfig
                LastWriteTime = if ($hasConfig) { (Get-Item -LiteralPath $configPath).LastWriteTime } else { [datetime]::MinValue }
            }
        }
    }
    $uniqueAccounts = @{}
    foreach ($account in $accounts) { $uniqueAccounts[$account.ConfigPath] = $account }
    @($uniqueAccounts.Values | Sort-Object @{ Expression = 'MostRecent'; Descending = $true }, @{ Expression = 'LastWriteTime'; Descending = $true }, PersonaName)
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
        Width = [int](Get-ConfigValue $file.Text 'setting.defaultres')
        Height = [int](Get-ConfigValue $file.Text 'setting.defaultresheight')
        Mode = Get-ConfigValue $file.Text 'setting.aspectratiomode'
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

function Update-VideoConfig {
    param(
        [string]$Path,
        [psobject]$Resolution,
        [bool]$CreateBackup = $true
    )
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) { throw "Configuration file not found: $Path" }
    $file = Get-TextFileInfo $Path

    # Validate and update every required entry in memory before touching the file.
    $updated = Set-ConfigValue $file.Text 'setting.defaultres' ([string]$Resolution.Width)
    $updated = Set-ConfigValue $updated 'setting.defaultresheight' ([string]$Resolution.Height)
    $updated = Set-ConfigValue $updated 'setting.aspectratiomode' ([string]$Resolution.Mode)
    if ($updated -ceq $file.Text) {
        return [pscustomobject]@{ Changed = $false; BackupPath = $null }
    }

    $stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
    $replaceBackupPath = "$Path.$stamp.bak"
    $suffix = 1
    while (Test-Path -LiteralPath $replaceBackupPath) {
        $replaceBackupPath = "$Path.$stamp-$suffix.bak"
        $suffix++
    }
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
    [pscustomobject]@{ Changed = $true; BackupPath = $backupPath }
}

function Read-AspectMode {
    while ($true) {
        Write-Info ''
        Write-Info 'Aspect ratio' Cyan
        Write-Info '  1  4:3 / 5:4'
        Write-Info '  2  16:9'
        Write-Info '  3  16:10'
        Write-Info '  X  Cancel' DarkGray
        $choice = (Read-Host 'Choose').Trim()
        switch ($choice.ToUpperInvariant()) {
            '1' { return '0' } '2' { return '1' } '3' { return '2' } 'X' { return $null }
            default { Write-Info 'Please enter 1, 2, 3, or X.' Yellow }
        }
    }
}

function Show-ConsoleEditor {
    param([string]$Path)
    $current = Get-CurrentConfig $Path
    Write-Info ''
    Write-Info '  CS2 VIDEO CONFIGURATION' Cyan
    Write-Info '  -----------------------' DarkCyan
    Write-Info "  Current: $($current.Width)x$($current.Height)  |  $(ConvertFrom-AspectMode $current.Mode)" Green
    Write-Info "  File:    $Path" DarkGray

    while ($true) {
        Write-Info ''
        Write-Info 'Available resolutions' Cyan
        for ($i = 0; $i -lt $script:Resolutions.Count; $i++) {
            $item = $script:Resolutions[$i]
            Write-Info ('  {0,2}  {1,-11} {2}' -f ($i + 1), ("$($item.Width)x$($item.Height)"), $item.Ratio)
        }
        Write-Info '   C  Custom resolution'
        Write-Info '   X  Cancel' DarkGray
        $choice = (Read-Host 'Choose').Trim()
        if ($choice -match '^[xX]$') { return $null }
        if ($choice -match '^[cC]$') { $resolution = Resolve-Resolution (Read-Host 'Enter WIDTHxHEIGHT') }
        else { $resolution = Resolve-Resolution $choice }
        if ($null -eq $resolution) { Write-Info 'That is not a valid selection.' Yellow; continue }
        if ([string]::IsNullOrEmpty($resolution.Mode)) { $resolution.Mode = Read-AspectMode }
        if (-not [string]::IsNullOrEmpty($resolution.Mode)) {
            Write-Info ''
            Write-Info "New setting: $($resolution.Display)  |  $(ConvertFrom-AspectMode $resolution.Mode)" Green
            $confirm = (Read-Host 'Apply this change? [Y/n]').Trim()
            if ([string]::IsNullOrEmpty($confirm) -or $confirm -match '^[yY]$') { return $resolution }
            if ($confirm -match '^[xX]$') { return $null }
        }
    }
}


function Show-ModernGraphicalEditor {
    param([object[]]$SteamAccounts, [string]$InitialPath)

    Add-Type -AssemblyName System.Windows.Forms
    Add-Type -AssemblyName System.Drawing
    [Windows.Forms.Application]::EnableVisualStyles()

    if (-not ('Cs2UiNativeWindow' -as [type])) {
        Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;

public static class Cs2UiNativeWindow
{
    [DllImport("dwmapi.dll")]
    private static extern int DwmSetWindowAttribute(IntPtr hwnd, int attribute, ref int value, int size);

    [DllImport("uxtheme.dll", CharSet = CharSet.Unicode)]
    private static extern int SetWindowTheme(IntPtr hwnd, string subAppName, string subIdList);

    public static void Apply(IntPtr handle)
    {
        int enabled = 1;
        int rounded = 2;
        try
        {
            if (DwmSetWindowAttribute(handle, 20, ref enabled, 4) != 0)
                DwmSetWindowAttribute(handle, 19, ref enabled, 4);
            DwmSetWindowAttribute(handle, 33, ref rounded, 4);
        }
        catch { }
    }

    public static void ApplyDarkControl(IntPtr handle)
    {
        try { SetWindowTheme(handle, "DarkMode_Explorer", null); }
        catch { }
    }
}
'@
    }

    $highContrast = [Windows.Forms.SystemInformation]::HighContrast
    if ($highContrast) {
        $theme = [pscustomobject]@{
            Background = [Drawing.SystemColors]::Window
            Card = [Drawing.SystemColors]::Control
            CardHover = [Drawing.SystemColors]::ControlLight
            Input = [Drawing.SystemColors]::Window
            Border = [Drawing.SystemColors]::WindowText
            Text = [Drawing.SystemColors]::WindowText
            Muted = [Drawing.SystemColors]::GrayText
            Primary = [Drawing.SystemColors]::Highlight
            PrimaryHover = [Drawing.SystemColors]::Highlight
            PrimaryText = [Drawing.SystemColors]::HighlightText
            SuccessBack = [Drawing.SystemColors]::Control
            SuccessText = [Drawing.SystemColors]::WindowText
            WarningBack = [Drawing.SystemColors]::Control
            WarningText = [Drawing.SystemColors]::WindowText
            ErrorBack = [Drawing.SystemColors]::Control
            ErrorText = [Drawing.SystemColors]::WindowText
        }
    } else {
        $theme = [pscustomobject]@{
            Background = [Drawing.Color]::FromArgb(24, 26, 31)
            Card = [Drawing.Color]::FromArgb(32, 35, 42)
            CardHover = [Drawing.Color]::FromArgb(42, 46, 55)
            Input = [Drawing.Color]::FromArgb(39, 43, 51)
            Border = [Drawing.Color]::FromArgb(58, 63, 72)
            Text = [Drawing.Color]::FromArgb(242, 244, 247)
            Muted = [Drawing.Color]::FromArgb(166, 174, 187)
            Primary = [Drawing.Color]::FromArgb(74, 130, 247)
            PrimaryHover = [Drawing.Color]::FromArgb(92, 145, 250)
            PrimaryText = [Drawing.Color]::White
            SuccessBack = [Drawing.Color]::FromArgb(24, 66, 46)
            SuccessText = [Drawing.Color]::FromArgb(134, 239, 172)
            WarningBack = [Drawing.Color]::FromArgb(69, 52, 25)
            WarningText = [Drawing.Color]::FromArgb(251, 191, 74)
            ErrorBack = [Drawing.Color]::FromArgb(70, 34, 38)
            ErrorText = [Drawing.Color]::FromArgb(251, 113, 133)
        }
    }

    $installedFonts = New-Object Drawing.Text.InstalledFontCollection
    $fontFamily = if ($installedFonts.Families.Name -contains 'Segoe UI Variable') { 'Segoe UI Variable' } else { 'Segoe UI' }
    $installedFonts.Dispose()
    $fontNormal = New-Object Drawing.Font($fontFamily, 10)
    $fontSmall = New-Object Drawing.Font($fontFamily, 8.5)
    $fontSection = New-Object Drawing.Font($fontFamily, 11, [Drawing.FontStyle]::Bold)
    $fontTitle = New-Object Drawing.Font($fontFamily, 19, [Drawing.FontStyle]::Bold)

    function New-UiRoundedPath {
        param([Drawing.Rectangle]$Bounds, [int]$Radius)
        $diameter = $Radius * 2
        $path = New-Object Drawing.Drawing2D.GraphicsPath
        if ($Bounds.Width -le $diameter -or $Bounds.Height -le $diameter) {
            $path.AddRectangle($Bounds)
            return $path
        }
        $path.AddArc($Bounds.Left, $Bounds.Top, $diameter, $diameter, 180, 90)
        $path.AddArc($Bounds.Right - $diameter, $Bounds.Top, $diameter, $diameter, 270, 90)
        $path.AddArc($Bounds.Right - $diameter, $Bounds.Bottom - $diameter, $diameter, $diameter, 0, 90)
        $path.AddArc($Bounds.Left, $Bounds.Bottom - $diameter, $diameter, $diameter, 90, 90)
        $path.CloseFigure()
        return $path
    }

    $setRoundedRegion = {
        param([Windows.Forms.Control]$Control, [int]$Radius)
        if ($Control.Width -le 1 -or $Control.Height -le 1) { return }
        $bounds = New-Object Drawing.Rectangle(0, 0, $Control.Width, $Control.Height)
        $path = New-UiRoundedPath $bounds $Radius
        $oldRegion = $Control.Region
        $Control.Region = New-Object Drawing.Region($path)
        $path.Dispose()
        if ($oldRegion) { $oldRegion.Dispose() }
    }

    $newCard = {
        $card = New-Object Windows.Forms.Panel
        $card.Dock = 'Fill'
        $card.BackColor = $theme.Card
        $card.ForeColor = $theme.Text
        $card.Padding = New-Object Windows.Forms.Padding(18, 14, 18, 14)
        $card.Margin = New-Object Windows.Forms.Padding(0)
        $card.Add_Resize({ param($sender, $eventArgs) & $setRoundedRegion $sender 12 })
        $card.Add_Paint({
            param($sender, $eventArgs)
            $eventArgs.Graphics.SmoothingMode = [Drawing.Drawing2D.SmoothingMode]::AntiAlias
            $bounds = New-Object Drawing.Rectangle(0, 0, ($sender.Width - 1), ($sender.Height - 1))
            $path = New-UiRoundedPath $bounds 12
            $pen = New-Object Drawing.Pen($theme.Border, 1)
            $eventArgs.Graphics.DrawPath($pen, $path)
            $pen.Dispose(); $path.Dispose()
        })
        return $card
    }

    $styleInput = {
        param([Windows.Forms.Control]$Control)
        $Control.BackColor = $theme.Input
        $Control.ForeColor = $theme.Text
        $Control.Font = $fontNormal
        if ($Control -is [Windows.Forms.ComboBox]) { $Control.FlatStyle = 'Flat' }
    }

    $form = New-Object Windows.Forms.Form
    $form.Text = 'CS2 Video Configuration'
    $form.StartPosition = 'CenterScreen'
    $form.ShowIcon = $false
    $form.MaximizeBox = $false
    $form.ClientSize = New-Object Drawing.Size(780, 660)
    $form.MinimumSize = New-Object Drawing.Size(640, 520)
    $form.Font = $fontNormal
    $form.BackColor = $theme.Background
    $form.ForeColor = $theme.Text
    $form.AutoScaleMode = 'Dpi'
    $darkNativeControls = New-Object System.Collections.Generic.List[Windows.Forms.Control]
    $form.Add_Shown({
        if (-not $highContrast) {
            [Cs2UiNativeWindow]::Apply($form.Handle)
            foreach ($control in $darkNativeControls) { [Cs2UiNativeWindow]::ApplyDarkControl($control.Handle) }
        }
        $workingArea = [Windows.Forms.Screen]::FromControl($form).WorkingArea
        if ($form.Width -gt ($workingArea.Width - 32)) { $form.Width = $workingArea.Width - 32 }
        if ($form.Height -gt ($workingArea.Height - 32)) { $form.Height = $workingArea.Height - 32 }
        $form.Left = $workingArea.Left + [math]::Max(16, [int](($workingArea.Width - $form.Width) / 2))
        $form.Top = $workingArea.Top + [math]::Max(16, [int](($workingArea.Height - $form.Height) / 2))
    })

    $root = New-Object Windows.Forms.TableLayoutPanel
    $root.Dock = 'Fill'; $root.ColumnCount = 1; $root.RowCount = 3; $root.Margin = New-Object Windows.Forms.Padding(0)
    [void]$root.RowStyles.Add((New-Object Windows.Forms.RowStyle('Absolute', 78)))
    [void]$root.RowStyles.Add((New-Object Windows.Forms.RowStyle('Percent', 100)))
    [void]$root.RowStyles.Add((New-Object Windows.Forms.RowStyle('Absolute', 72)))
    $form.Controls.Add($root)

    $header = New-Object Windows.Forms.Panel
    $header.Dock = 'Fill'; $header.BackColor = $theme.Background; $header.Padding = New-Object Windows.Forms.Padding(28, 13, 28, 10)
    $root.Controls.Add($header, 0, 0)
    $title = New-Object Windows.Forms.Label
    $title.Text = 'CS2 Video Configuration'; $title.Font = $fontTitle; $title.ForeColor = $theme.Text; $title.AutoSize = $true; $title.Location = New-Object Drawing.Point(25, 12)
    $header.Controls.Add($title)
    $subtitle = New-Object Windows.Forms.Label
    $subtitle.Text = 'Select an account, choose display settings, and apply safely.'; $subtitle.Font = $fontNormal; $subtitle.ForeColor = $theme.Muted; $subtitle.AutoSize = $true; $subtitle.Location = New-Object Drawing.Point(28, 48)
    $header.Controls.Add($subtitle)
    $headerRule = New-Object Windows.Forms.Panel
    $headerRule.Dock = 'Bottom'; $headerRule.Height = 1; $headerRule.BackColor = $theme.Border
    $header.Controls.Add($headerRule)

    $body = New-Object Windows.Forms.TableLayoutPanel
    $body.Dock = 'Fill'; $body.ColumnCount = 1; $body.RowCount = 5; $body.Padding = New-Object Windows.Forms.Padding(24, 16, 24, 12); $body.AutoScroll = $true
    [void]$body.RowStyles.Add((New-Object Windows.Forms.RowStyle('Absolute', 220)))
    [void]$body.RowStyles.Add((New-Object Windows.Forms.RowStyle('Absolute', 12)))
    [void]$body.RowStyles.Add((New-Object Windows.Forms.RowStyle('Absolute', 188)))
    [void]$body.RowStyles.Add((New-Object Windows.Forms.RowStyle('Absolute', 12)))
    [void]$body.RowStyles.Add((New-Object Windows.Forms.RowStyle('Percent', 100)))
    $root.Controls.Add($body, 0, 1)

    $accountCard = & $newCard
    $body.Controls.Add($accountCard, 0, 0)
    $accountLayout = New-Object Windows.Forms.TableLayoutPanel
    $accountLayout.Dock = 'Fill'; $accountLayout.ColumnCount = 2; $accountLayout.RowCount = 5; $accountLayout.Margin = New-Object Windows.Forms.Padding(0)
    [void]$accountLayout.ColumnStyles.Add((New-Object Windows.Forms.ColumnStyle('Percent', 100)))
    [void]$accountLayout.ColumnStyles.Add((New-Object Windows.Forms.ColumnStyle('Absolute', 100)))
    foreach ($height in @(28, 48, 20, 40, 36)) { [void]$accountLayout.RowStyles.Add((New-Object Windows.Forms.RowStyle('Absolute', $height))) }
    $accountCard.Controls.Add($accountLayout)

    $accountTitle = New-Object Windows.Forms.Label
    $accountTitle.Text = 'Steam account'; $accountTitle.Font = $fontSection; $accountTitle.ForeColor = $theme.Text; $accountTitle.Dock = 'Fill'; $accountTitle.TextAlign = 'MiddleLeft'
    $accountLayout.Controls.Add($accountTitle, 0, 0); $accountLayout.SetColumnSpan($accountTitle, 2)

    $accountBox = New-Object Windows.Forms.ComboBox
    $accountBox.Dock = 'Fill'; $accountBox.DropDownStyle = 'DropDownList'; $accountBox.DisplayMember = 'DisplayName'; $accountBox.DrawMode = 'OwnerDrawFixed'
    $accountBox.ItemHeight = 38; $accountBox.IntegralHeight = $false; $accountBox.DropDownHeight = 240; $accountBox.DropDownWidth = 690
    & $styleInput $accountBox
    [void]$darkNativeControls.Add($accountBox)
    foreach ($account in $SteamAccounts) { [void]$accountBox.Items.Add($account) }
    $accountBox.Add_DrawItem({
        param($sender, $eventArgs)
        if ($eventArgs.Index -lt 0 -or $eventArgs.Index -ge $sender.Items.Count) { return }
        $account = $sender.Items[$eventArgs.Index]
        $selected = ($eventArgs.State -band [Windows.Forms.DrawItemState]::Selected) -eq [Windows.Forms.DrawItemState]::Selected
        $back = if ($selected) { $theme.Primary } else { $theme.Input }
        $fore = if ($selected) { $theme.PrimaryText } else { $theme.Text }
        $muted = if ($selected) { $theme.PrimaryText } else { $theme.Muted }
        $brush = New-Object Drawing.SolidBrush($back)
        $eventArgs.Graphics.FillRectangle($brush, $eventArgs.Bounds); $brush.Dispose()
        $primaryBrush = New-Object Drawing.SolidBrush($fore)
        $secondaryBrush = New-Object Drawing.SolidBrush($muted)
        $format = New-Object Drawing.StringFormat
        $format.Trimming = [Drawing.StringTrimming]::EllipsisCharacter; $format.FormatFlags = [Drawing.StringFormatFlags]::NoWrap
        $primaryBounds = New-Object Drawing.RectangleF(($eventArgs.Bounds.X + 10), ($eventArgs.Bounds.Y + 3), ($eventArgs.Bounds.Width - 20), 18)
        $secondaryBounds = New-Object Drawing.RectangleF(($eventArgs.Bounds.X + 10), ($eventArgs.Bounds.Y + 21), ($eventArgs.Bounds.Width - 20), 15)
        $eventArgs.Graphics.DrawString([string]$account.PersonaName, $fontNormal, $primaryBrush, $primaryBounds, $format)
        $details = "Account ID $($account.AccountId)  |  SteamID64 $($account.SteamId64)"
        $eventArgs.Graphics.DrawString($details, $fontSmall, $secondaryBrush, $secondaryBounds, $format)
        $primaryBrush.Dispose(); $secondaryBrush.Dispose(); $format.Dispose()
        $eventArgs.DrawFocusRectangle()
    })
    $accountBox.Add_Resize({ param($sender, $eventArgs) $sender.DropDownWidth = [math]::Max(420, $sender.Width) })
    $accountLayout.Controls.Add($accountBox, 0, 1); $accountLayout.SetColumnSpan($accountBox, 2)

    $pathLabel = New-Object Windows.Forms.Label
    $pathLabel.Text = 'Configuration file'; $pathLabel.Font = $fontSmall; $pathLabel.ForeColor = $theme.Muted; $pathLabel.Dock = 'Fill'; $pathLabel.TextAlign = 'BottomLeft'
    $accountLayout.Controls.Add($pathLabel, 0, 2); $accountLayout.SetColumnSpan($pathLabel, 2)
    $pathBox = New-Object Windows.Forms.TextBox
    $pathBox.Dock = 'Fill'; $pathBox.BorderStyle = 'FixedSingle'; $pathBox.Margin = New-Object Windows.Forms.Padding(0, 3, 8, 3)
    & $styleInput $pathBox
    [void]$darkNativeControls.Add($pathBox)
    if ($InitialPath) { $pathBox.Text = $InitialPath }
    $accountLayout.Controls.Add($pathBox, 0, 3)
    $browse = New-Object Windows.Forms.Button
    $browse.Text = 'Browse...'; $browse.Dock = 'Fill'; $browse.Margin = New-Object Windows.Forms.Padding(0, 3, 0, 3)
    $browse.FlatStyle = 'Flat'; $browse.FlatAppearance.BorderColor = $theme.Border; $browse.FlatAppearance.MouseOverBackColor = $theme.CardHover; $browse.FlatAppearance.MouseDownBackColor = $theme.Border; $browse.BackColor = $theme.Input; $browse.ForeColor = $theme.Text; $browse.UseVisualStyleBackColor = $false
    $browse.Add_MouseEnter({ $browse.BackColor = $theme.CardHover }); $browse.Add_MouseLeave({ $browse.BackColor = $theme.Input })
    $accountLayout.Controls.Add($browse, 1, 3)

    $currentLabel = New-Object Windows.Forms.Label
    $currentLabel.Text = 'Checking configuration...'; $currentLabel.Dock = 'Fill'; $currentLabel.Margin = New-Object Windows.Forms.Padding(0, 4, 0, 0)
    $currentLabel.Padding = New-Object Windows.Forms.Padding(10, 0, 10, 0); $currentLabel.TextAlign = 'MiddleLeft'; $currentLabel.BackColor = $theme.CardHover; $currentLabel.ForeColor = $theme.Muted
    $currentLabel.Add_Resize({ param($sender, $eventArgs) & $setRoundedRegion $sender 7 })
    $accountLayout.Controls.Add($currentLabel, 0, 4); $accountLayout.SetColumnSpan($currentLabel, 2)

    $displayCard = & $newCard
    $body.Controls.Add($displayCard, 0, 2)
    $displayLayout = New-Object Windows.Forms.TableLayoutPanel
    $displayLayout.Dock = 'Fill'; $displayLayout.ColumnCount = 2; $displayLayout.RowCount = 4; $displayLayout.Margin = New-Object Windows.Forms.Padding(0)
    [void]$displayLayout.ColumnStyles.Add((New-Object Windows.Forms.ColumnStyle('Absolute', 140)))
    [void]$displayLayout.ColumnStyles.Add((New-Object Windows.Forms.ColumnStyle('Percent', 100)))
    foreach ($height in @(28, 42, 42, 42)) { [void]$displayLayout.RowStyles.Add((New-Object Windows.Forms.RowStyle('Absolute', $height))) }
    $displayCard.Controls.Add($displayLayout)

    $displayTitle = New-Object Windows.Forms.Label
    $displayTitle.Text = 'Display settings'; $displayTitle.Font = $fontSection; $displayTitle.ForeColor = $theme.Text; $displayTitle.Dock = 'Fill'; $displayTitle.TextAlign = 'MiddleLeft'
    $displayLayout.Controls.Add($displayTitle, 0, 0); $displayLayout.SetColumnSpan($displayTitle, 2)

    $aspectLabel = New-Object Windows.Forms.Label
    $aspectLabel.Text = 'Aspect ratio'; $aspectLabel.ForeColor = $theme.Muted; $aspectLabel.Dock = 'Fill'; $aspectLabel.TextAlign = 'MiddleLeft'
    $displayLayout.Controls.Add($aspectLabel, 0, 1)
    $aspectPanel = New-Object Windows.Forms.TableLayoutPanel
    $aspectPanel.Dock = 'Fill'; $aspectPanel.ColumnCount = 3; $aspectPanel.RowCount = 1; $aspectPanel.Margin = New-Object Windows.Forms.Padding(0, 3, 0, 3)
    for ($i = 0; $i -lt 3; $i++) { [void]$aspectPanel.ColumnStyles.Add((New-Object Windows.Forms.ColumnStyle('Percent', 33.333))) }
    $displayLayout.Controls.Add($aspectPanel, 1, 1)
    $ratioButtons = @{}
    $ratioDefinitions = @(
        [pscustomobject]@{ Text = '4:3 / 5:4'; Mode = '0' },
        [pscustomobject]@{ Text = '16:9'; Mode = '1' },
        [pscustomobject]@{ Text = '16:10'; Mode = '2' }
    )
    $updateRatioStyles = {
        foreach ($button in $ratioButtons.Values) {
            if ($button.Checked) {
                $button.BackColor = $theme.Primary; $button.ForeColor = $theme.PrimaryText; $button.FlatAppearance.BorderColor = $theme.Primary
            } else {
                $button.BackColor = $theme.Input; $button.ForeColor = $theme.Text; $button.FlatAppearance.BorderColor = $theme.Border
            }
        }
    }
    foreach ($definition in $ratioDefinitions) {
        $radio = New-Object Windows.Forms.RadioButton
        $radio.Text = $definition.Text; $radio.Tag = $definition.Mode; $radio.Appearance = 'Button'; $radio.Dock = 'Fill'; $radio.TextAlign = 'MiddleCenter'
        $radio.FlatStyle = 'Flat'; $radio.FlatAppearance.BorderSize = 1; $radio.FlatAppearance.MouseOverBackColor = $theme.CardHover; $radio.FlatAppearance.MouseDownBackColor = $theme.Border; $radio.Margin = New-Object Windows.Forms.Padding(0, 0, 6, 0); $radio.UseVisualStyleBackColor = $false
        $radio.AccessibleName = "Aspect ratio $($definition.Text)"
        $radio.Add_MouseEnter({ param($sender, $eventArgs) if (-not $sender.Checked) { $sender.BackColor = $theme.CardHover } })
        $radio.Add_MouseLeave({ param($sender, $eventArgs) & $updateRatioStyles })
        $ratioButtons[$definition.Mode] = $radio
        $aspectPanel.Controls.Add($radio, [int]$definition.Mode, 0)
    }

    $resolutionLabel = New-Object Windows.Forms.Label
    $resolutionLabel.Text = 'Resolution'; $resolutionLabel.ForeColor = $theme.Muted; $resolutionLabel.Dock = 'Fill'; $resolutionLabel.TextAlign = 'MiddleLeft'
    $displayLayout.Controls.Add($resolutionLabel, 0, 2)
    $resolutionBox = New-Object Windows.Forms.ComboBox
    $resolutionBox.Dock = 'Fill'; $resolutionBox.DropDownStyle = 'DropDownList'; $resolutionBox.Margin = New-Object Windows.Forms.Padding(0, 4, 0, 4)
    & $styleInput $resolutionBox
    [void]$darkNativeControls.Add($resolutionBox)
    $displayLayout.Controls.Add($resolutionBox, 1, 2)

    $customLabel = New-Object Windows.Forms.Label
    $customLabel.Text = 'Custom size'; $customLabel.ForeColor = $theme.Muted; $customLabel.Dock = 'Fill'; $customLabel.TextAlign = 'MiddleLeft'
    $displayLayout.Controls.Add($customLabel, 0, 3)
    $customPanel = New-Object Windows.Forms.FlowLayoutPanel
    $customPanel.Dock = 'Fill'; $customPanel.FlowDirection = 'LeftToRight'; $customPanel.WrapContents = $false; $customPanel.Margin = New-Object Windows.Forms.Padding(0, 4, 0, 2)
    $widthBox = New-Object Windows.Forms.NumericUpDown
    $widthBox.Minimum = 320; $widthBox.Maximum = 32768; $widthBox.Value = 1920; $widthBox.Width = 130; $widthBox.BorderStyle = 'FixedSingle'
    & $styleInput $widthBox
    [void]$darkNativeControls.Add($widthBox)
    $timesLabel = New-Object Windows.Forms.Label
    $timesLabel.Text = ' x '; $timesLabel.ForeColor = $theme.Muted; $timesLabel.AutoSize = $true; $timesLabel.Padding = New-Object Windows.Forms.Padding(8, 5, 8, 0)
    $heightBox = New-Object Windows.Forms.NumericUpDown
    $heightBox.Minimum = 200; $heightBox.Maximum = 32768; $heightBox.Value = 1080; $heightBox.Width = 130; $heightBox.BorderStyle = 'FixedSingle'
    & $styleInput $heightBox
    [void]$darkNativeControls.Add($heightBox)
    $customPanel.Controls.Add($widthBox); $customPanel.Controls.Add($timesLabel); $customPanel.Controls.Add($heightBox)
    $displayLayout.Controls.Add($customPanel, 1, 3)
    $customHint = New-Object Windows.Forms.Label
    $customHint.Text = 'Choose Custom... to enter an exact width and height.'; $customHint.Dock = 'Fill'; $customHint.TextAlign = 'MiddleLeft'; $customHint.ForeColor = $theme.Muted
    $displayLayout.Controls.Add($customHint, 1, 3)

    $statusLabel = New-Object Windows.Forms.Label
    $statusLabel.Dock = 'Fill'; $statusLabel.Margin = New-Object Windows.Forms.Padding(0); $statusLabel.Padding = New-Object Windows.Forms.Padding(14, 0, 14, 0)
    $statusLabel.TextAlign = 'MiddleLeft'; $statusLabel.BackColor = $theme.CardHover; $statusLabel.ForeColor = $theme.Muted; $statusLabel.AutoEllipsis = $true
    $statusLabel.Add_Resize({ param($sender, $eventArgs) & $setRoundedRegion $sender 8 })
    $body.Controls.Add($statusLabel, 0, 4)
    & $setRoundedRegion $statusLabel 8
    $statusToolTip = New-Object Windows.Forms.ToolTip
    $setStatus = {
        param([string]$Text, [string]$Kind = 'Neutral', [string]$Details = '')
        $statusLabel.Text = $Text
        switch ($Kind) {
            'Success' { $statusLabel.BackColor = $theme.SuccessBack; $statusLabel.ForeColor = $theme.SuccessText }
            'Warning' { $statusLabel.BackColor = $theme.WarningBack; $statusLabel.ForeColor = $theme.WarningText }
            'Error'   { $statusLabel.BackColor = $theme.ErrorBack; $statusLabel.ForeColor = $theme.ErrorText }
            default   { $statusLabel.BackColor = $theme.CardHover; $statusLabel.ForeColor = $theme.Muted }
        }
        $statusToolTip.SetToolTip($statusLabel, $(if ($Details) { $Details } else { $Text }))
        $statusLabel.AccessibleName = "Status: $Text"
        $statusLabel.AccessibleDescription = $(if ($Details) { "$Text $Details" } else { $Text })
    }

    $footer = New-Object Windows.Forms.TableLayoutPanel
    $footer.Dock = 'Fill'; $footer.ColumnCount = 2; $footer.RowCount = 1; $footer.Padding = New-Object Windows.Forms.Padding(24, 14, 24, 14); $footer.BackColor = $theme.Card
    [void]$footer.ColumnStyles.Add((New-Object Windows.Forms.ColumnStyle('Percent', 100)))
    [void]$footer.ColumnStyles.Add((New-Object Windows.Forms.ColumnStyle('Absolute', 238)))
    [void]$footer.RowStyles.Add((New-Object Windows.Forms.RowStyle('Percent', 100)))
    $root.Controls.Add($footer, 0, 2)
    $backupCheck = New-Object Windows.Forms.CheckBox
    $backupCheck.Text = 'Create a timestamped backup'; $backupCheck.Checked = -not $NoBackup; $backupCheck.Dock = 'Fill'; $backupCheck.ForeColor = $theme.Text
    $backupCheck.BackColor = $theme.Card; $backupCheck.FlatStyle = 'Flat'; $backupCheck.UseVisualStyleBackColor = $false; $backupCheck.AutoSize = $false
    $backupCheck.CheckAlign = 'MiddleLeft'; $backupCheck.TextAlign = 'MiddleLeft'
    $footer.Controls.Add($backupCheck, 0, 0)
    $buttonPanel = New-Object Windows.Forms.FlowLayoutPanel
    $buttonPanel.Dock = 'Fill'; $buttonPanel.FlowDirection = 'RightToLeft'; $buttonPanel.WrapContents = $false; $buttonPanel.Margin = New-Object Windows.Forms.Padding(0); $buttonPanel.BackColor = $theme.Card
    $apply = New-Object Windows.Forms.Button
    $apply.Text = 'Apply changes'; $apply.Width = 132; $apply.Height = 40; $apply.FlatStyle = 'Flat'; $apply.FlatAppearance.BorderSize = 0; $apply.UseVisualStyleBackColor = $false; $apply.Enabled = $false
    $cancel = New-Object Windows.Forms.Button
    $cancel.Text = 'Cancel'; $cancel.Width = 92; $cancel.Height = 40; $cancel.FlatStyle = 'Flat'; $cancel.FlatAppearance.BorderColor = $theme.Border; $cancel.FlatAppearance.MouseOverBackColor = $theme.CardHover; $cancel.FlatAppearance.MouseDownBackColor = $theme.Border; $cancel.BackColor = $theme.Input; $cancel.ForeColor = $theme.Text; $cancel.UseVisualStyleBackColor = $false; $cancel.DialogResult = 'Cancel'
    $cancel.Add_MouseEnter({ $cancel.BackColor = $theme.CardHover }); $cancel.Add_MouseLeave({ $cancel.BackColor = $theme.Input })
    $buttonPanel.Controls.Add($apply); $buttonPanel.Controls.Add($cancel)
    $footer.Controls.Add($buttonPanel, 1, 0)
    $updateApplyStyle = {
        if ($apply.Enabled) { $apply.BackColor = $theme.Primary; $apply.ForeColor = $theme.PrimaryText }
        else { $apply.BackColor = $theme.Border; $apply.ForeColor = $theme.Muted }
    }
    $apply.Add_EnabledChanged($updateApplyStyle)
    $apply.Add_MouseEnter({ if ($apply.Enabled) { $apply.BackColor = $theme.PrimaryHover } })
    $apply.Add_MouseLeave($updateApplyStyle)
    & $updateApplyStyle

    $form.CancelButton = $cancel; $form.AcceptButton = $apply
    $accountBox.TabIndex = 0; $pathBox.TabIndex = 1; $browse.TabIndex = 2
    $ratioButtons['0'].TabIndex = 3; $ratioButtons['1'].TabIndex = 4; $ratioButtons['2'].TabIndex = 5
    $resolutionBox.TabIndex = 6; $widthBox.TabIndex = 7; $heightBox.TabIndex = 8; $backupCheck.TabIndex = 9; $apply.TabIndex = 10; $cancel.TabIndex = 11
    $accountBox.AccessibleName = 'Steam account'; $pathBox.AccessibleName = 'CS2 configuration file'; $resolutionBox.AccessibleName = 'Resolution'
    $widthBox.AccessibleName = 'Custom width'; $heightBox.AccessibleName = 'Custom height'

    $getSelectedMode = {
        foreach ($mode in @('0', '1', '2')) { if ($ratioButtons[$mode].Checked) { return $mode } }
        return $null
    }
    $refreshResolutions = {
        $mode = & $getSelectedMode
        if ($null -eq $mode) { return }
        $previous = [string]$resolutionBox.SelectedItem
        $resolutionBox.Items.Clear()
        foreach ($item in $script:Resolutions | Where-Object Mode -eq $mode) { [void]$resolutionBox.Items.Add("$($item.Width)x$($item.Height)") }
        [void]$resolutionBox.Items.Add('Custom...')
        if ($previous -and $resolutionBox.Items.Contains($previous)) { $resolutionBox.SelectedItem = $previous }
        elseif ($resolutionBox.Items.Count -gt 0) { $resolutionBox.SelectedIndex = 0 }
    }
    $refreshCurrent = {
        try {
            $config = Get-CurrentConfig $pathBox.Text
            $currentLabel.Text = "Current  |  $($config.Width)x$($config.Height)  |  $(ConvertFrom-AspectMode $config.Mode)"
            $currentLabel.BackColor = $theme.SuccessBack; $currentLabel.ForeColor = $theme.SuccessText
            if ($ratioButtons.ContainsKey([string]$config.Mode)) { $ratioButtons[[string]$config.Mode].Checked = $true }
            $wanted = "$($config.Width)x$($config.Height)"
            if ($resolutionBox.Items.Contains($wanted)) { $resolutionBox.SelectedItem = $wanted }
            else { $resolutionBox.SelectedItem = 'Custom...'; $widthBox.Value = $config.Width; $heightBox.Value = $config.Height }
            $apply.Enabled = $true
            & $setStatus 'Configuration loaded and ready.' 'Neutral'
        } catch {
            $currentLabel.Text = 'Configuration file is not valid or cannot be read.'
            $currentLabel.BackColor = $theme.ErrorBack; $currentLabel.ForeColor = $theme.ErrorText
            $apply.Enabled = $false
            & $setStatus $_.Exception.Message 'Error'
        }
    }

    foreach ($radio in $ratioButtons.Values) {
        $radio.Add_CheckedChanged({
            param($sender, $eventArgs)
            & $updateRatioStyles
            if ($sender.Checked) { & $refreshResolutions }
        })
    }
    $resolutionBox.Add_SelectedIndexChanged({
        $custom = $resolutionBox.SelectedItem -eq 'Custom...'
        $customLabel.Visible = $custom; $customPanel.Visible = $custom
        $customLabel.Enabled = $custom; $customPanel.Enabled = $custom
        $customHint.Visible = -not $custom
    })
    $accountToolTip = New-Object Windows.Forms.ToolTip
    $accountBox.Add_SelectionChangeCommitted({
        if ($accountBox.SelectedItem) {
            $pathBox.Text = $accountBox.SelectedItem.ConfigPath
            $accountDetails = "$($accountBox.SelectedItem.PersonaName) | Account ID $($accountBox.SelectedItem.AccountId) | SteamID64 $($accountBox.SelectedItem.SteamId64)"
            $accountToolTip.SetToolTip($accountBox, $accountDetails); $accountBox.AccessibleDescription = $accountDetails
            & $refreshCurrent
        }
    })
    $pathBox.Add_TextChanged({
        $apply.Enabled = $false
        $currentLabel.Text = 'Configuration path changed; leave the field to validate.'
        $currentLabel.BackColor = $theme.CardHover; $currentLabel.ForeColor = $theme.Muted
        & $setStatus 'Waiting to validate the configuration path.' 'Neutral'
    })
    $pathBox.Add_Leave($refreshCurrent)
    $browse.Add_Click({
        $dialog = $null
        try {
            $dialog = New-Object Windows.Forms.OpenFileDialog
            $dialog.Title = 'Select the CS2 video configuration'; $dialog.Filter = 'CS2 video config (cs2_video.txt)|cs2_video.txt|Text files (*.txt)|*.txt|All files (*.*)|*.*'
            if ($pathBox.Text) {
                $directory = [IO.Path]::GetDirectoryName($pathBox.Text)
                if ($directory -and (Test-Path -LiteralPath $directory -PathType Container)) { $dialog.InitialDirectory = $directory }
            }
            if ($dialog.ShowDialog($form) -eq 'OK') { $pathBox.Text = $dialog.FileName; & $refreshCurrent }
        } catch { & $setStatus $_.Exception.Message 'Error' }
        finally { if ($dialog) { $dialog.Dispose() } }
    })
    $apply.Add_Click({
        try {
            $path = $pathBox.Text.Trim()
            [void](Get-CurrentConfig $path)
            $mode = & $getSelectedMode
            if ($resolutionBox.SelectedItem -eq 'Custom...') { $selected = New-Resolution ([int]$widthBox.Value) ([int]$heightBox.Value) $mode }
            else { $selected = Resolve-Resolution ([string]$resolutionBox.SelectedItem); $selected.Mode = $mode }
            if (Get-Process -Name 'cs2' -ErrorAction SilentlyContinue) {
                & $setStatus 'CS2 is running and may overwrite this change.' 'Warning'
                $answer = [Windows.Forms.MessageBox]::Show('CS2 is running and may overwrite this change when it closes. Apply anyway?', 'CS2 is running', 'YesNo', 'Warning')
                if ($answer -ne 'Yes') { return }
            }
            $updateResult = Update-VideoConfig -Path $path -Resolution $selected -CreateBackup ([bool]$backupCheck.Checked)
            $form.Tag = [pscustomobject]@{ Path = $path; Resolution = $selected; Result = $updateResult }
            & $refreshCurrent
            if ($updateResult.Changed) {
                $details = if ($updateResult.BackupPath) { "Backup: $($updateResult.BackupPath)" } else { '' }
                & $setStatus "Changes applied: $($selected.Display)." 'Success' $details
            } else { & $setStatus 'The selected settings are already active.' 'Success' }
        } catch {
            $currentLabel.Text = 'Configuration file is not valid or cannot be read.'
            $currentLabel.BackColor = $theme.ErrorBack; $currentLabel.ForeColor = $theme.ErrorText
            $apply.Enabled = $false
            & $setStatus $_.Exception.Message 'Error'
        }
    })

    if ($accountBox.Items.Count -gt 0) {
        $selectedAccount = $SteamAccounts | Where-Object ConfigPath -eq $InitialPath | Select-Object -First 1
        if ($selectedAccount) { $accountBox.SelectedItem = $selectedAccount } else { $accountBox.SelectedIndex = 0 }
        $accountDetails = "$($accountBox.SelectedItem.PersonaName) | Account ID $($accountBox.SelectedItem.AccountId) | SteamID64 $($accountBox.SelectedItem.SteamId64)"
        $accountToolTip.SetToolTip($accountBox, $accountDetails); $accountBox.AccessibleDescription = $accountDetails
    }
    $ratioButtons['1'].Checked = $true
    & $updateRatioStyles
    & $refreshCurrent
    [void]$form.ShowDialog()
    $result = $form.Tag
    $statusToolTip.Dispose(); $accountToolTip.Dispose(); $form.Dispose()
    $fontNormal.Dispose(); $fontSmall.Dispose(); $fontSection.Dispose(); $fontTitle.Dispose()
    return $result
}

try {
    if ($Silent -and -not $Preset -and -not $ListAccounts) { throw '-Silent requires -Preset because interactive interfaces cannot be silent.' }
    $steamAccounts = Get-SteamAccounts
    $discoveredPaths = @($steamAccounts | Where-Object HasConfig | Select-Object -ExpandProperty ConfigPath)
    if ($ListAccounts) {
        $steamAccounts | Select-Object PersonaName, AccountId, SteamId64, HasConfig, ConfigPath
        exit 0
    }
    if ($PSBoundParameters.ContainsKey('FilePath')) { $script:SelectedConfigPath = [IO.Path]::GetFullPath($FilePath) }
    elseif ($discoveredPaths.Count -gt 0) { $script:SelectedConfigPath = $discoveredPaths[0] }

    if ($Preset) {
        if (-not $script:SelectedConfigPath) { throw 'No CS2 configuration was found. Specify -FilePath.' }
        $resolution = Resolve-Resolution $Preset
        if ($null -eq $resolution) { throw "Invalid preset '$Preset'. Use WIDTHxHEIGHT or a predefined menu number." }
        if ($AspectRatioMode) { $resolution.Mode = ConvertTo-AspectMode $AspectRatioMode }
        if ([string]::IsNullOrEmpty($resolution.Mode)) { throw "Cannot infer the aspect ratio for $($resolution.Display). Specify -AspectRatioMode." }
        $appliedResolution = $resolution
        $result = Update-VideoConfig $script:SelectedConfigPath $appliedResolution (-not $NoBackup)
    } elseif ($Console) {
        if (-not $script:SelectedConfigPath) { throw 'No CS2 configuration was found. Specify -FilePath.' }
        $resolution = Show-ConsoleEditor $script:SelectedConfigPath
        if ($null -eq $resolution) { Write-Info 'Cancelled.' DarkGray; exit 0 }
        if ($AspectRatioMode) { $resolution.Mode = ConvertTo-AspectMode $AspectRatioMode }
        $appliedResolution = $resolution
        $result = Update-VideoConfig $script:SelectedConfigPath $appliedResolution (-not $NoBackup)
    } else {
        if ($script:SelectedConfigPath -and $script:SelectedConfigPath -notin $discoveredPaths) {
            $steamAccounts = @(
                [pscustomobject]@{
                    DisplayName = 'Custom configuration file'
                    AccountId = $null; SteamId64 = $null; PersonaName = 'Custom file'; AccountName = $null
                    MostRecent = $false; ConfigPath = $script:SelectedConfigPath
                    HasConfig = (Test-Path -LiteralPath $script:SelectedConfigPath -PathType Leaf); LastWriteTime = [datetime]::MinValue
                }
            ) + $steamAccounts
        }
        $dialogOutput = @(Show-ModernGraphicalEditor $steamAccounts $script:SelectedConfigPath)
        $selection = $dialogOutput | Where-Object {
            $null -ne $_ -and
            $null -ne $_.PSObject.Properties['Path'] -and
            $null -ne $_.PSObject.Properties['Resolution'] -and
            $null -ne $_.PSObject.Properties['Result']
        } | Select-Object -Last 1
        if ($null -eq $selection) { exit 0 }
        $script:SelectedConfigPath = $selection.Path
        $appliedResolution = $selection.Resolution
        $result = $selection.Result
    }

    if ($result.Changed) {
        Write-Info "Updated $script:SelectedConfigPath to $($appliedResolution.Display)." Green
        if ($result.BackupPath) { Write-Info "Backup: $($result.BackupPath)" DarkGray }
    } else { Write-Info 'The selected settings are already active.' DarkGray }
    exit 0
} catch {
    if (-not $Preset -and -not $Console) {
        try {
            Add-Type -AssemblyName System.Windows.Forms
            [void][Windows.Forms.MessageBox]::Show($_.Exception.Message, 'CS2 Video Configuration', 'OK', 'Error')
        } catch { }
    }
    Write-Error $_.Exception.Message -ErrorAction Continue
    exit 1
}
