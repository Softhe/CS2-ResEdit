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
.PARAMETER ExportDiagnostics
    Writes a privacy-safe JSON diagnostic report and exits without changing any
    CS2 configuration or graphical-interface preferences.
.EXAMPLE
    .\CS2-VideoConfig-Editor.ps1
.EXAMPLE
    .\CS2-VideoConfig-Editor.ps1 -Console
.EXAMPLE
    .\CS2-VideoConfig-Editor.ps1 -Preset 1920x1080 -Silent
.EXAMPLE
    .\CS2-VideoConfig-Editor.ps1 -ExportDiagnostics .\cs2-diagnostics.json
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
    [switch]$ListAccounts,

    [Parameter()]
    [ValidateNotNullOrEmpty()]
    [string]$ExportDiagnostics
)

$ErrorActionPreference = 'Stop'
$script:ApplicationVersion = '3.0.0'
$script:SelectedConfigPath = $null
$script:ModuleRoot = Join-Path $PSScriptRoot 'modules'
foreach ($moduleName in @(
    'Cs2.VideoConfig.psm1',
    'Cs2.Steam.psm1',
    'Cs2.Preferences.psm1',
    'Cs2.Diagnostics.psm1'
)) {
    $modulePath = Join-Path $script:ModuleRoot $moduleName
    if (-not (Test-Path -LiteralPath $modulePath -PathType Leaf)) {
        throw "Required application module was not found: $modulePath"
    }
    Import-Module $modulePath -Force -DisableNameChecking -ErrorAction Stop
}
$script:Resolutions = @(Get-Cs2ResolutionPresets)
function Write-Info {
    param([string]$Message, [ConsoleColor]$Color = 'Gray')
    if (-not $Silent) { Write-Host $Message -ForegroundColor $Color }
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

function Initialize-Cs2GraphicalInterface {
    Add-Type -AssemblyName System.Windows.Forms
    Add-Type -AssemblyName System.Drawing
    [Windows.Forms.Application]::EnableVisualStyles()

    if ('Cs2UiNativeWindow' -as [type]) { return }
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

function Get-Cs2UiTheme {
    param([bool]$HighContrast)
    if ($HighContrast) {
        return [pscustomobject]@{
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
    }

    [pscustomobject]@{
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

function New-Cs2UiRoundedPath {
    param($Bounds, [int]$Radius)
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
    $path
}

function Set-Cs2UiRoundedRegion {
    param($Control, [int]$Radius)
    if ($Control.Width -le 1 -or $Control.Height -le 1) { return }
    # A resized WinForms Panel does not repaint its full surface by default.
    # With a custom Region and border, old bottom edges otherwise remain
    # visible as horizontal "ghost" lines after repeated resizing.
    $nonPublicInstance = [Reflection.BindingFlags]'Instance, NonPublic'
    foreach ($propertyName in @('DoubleBuffered', 'ResizeRedraw')) {
        try {
            $property = [Windows.Forms.Control].GetProperty($propertyName, $nonPublicInstance)
            if ($property) { $property.SetValue($Control, $true, $null) }
        } catch { }
    }
    $bounds = New-Object Drawing.Rectangle(0, 0, $Control.Width, $Control.Height)
    $path = New-Cs2UiRoundedPath $bounds $Radius
    $oldRegion = $Control.Region
    $Control.Region = New-Object Drawing.Region($path)
    $path.Dispose()
    if ($oldRegion) { $oldRegion.Dispose() }
    $Control.Invalidate($true)
    if ($Control.Parent) { $Control.Parent.Invalidate($Control.Bounds, $true) }
}

function Get-Cs2PendingState {
    param($Current, $Pending, [bool]$PathValid)
    $changed = $PathValid -and $null -ne $Current -and $null -ne $Pending -and
        -not (Test-Cs2ConfigEquals $Current $Pending)
    [pscustomobject]@{
        PathValid = $PathValid
        Changed = [bool]$changed
        CanApply = [bool]$changed
        CanReset = [bool]$changed
        AlreadyActive = $PathValid -and $null -ne $Current -and $null -ne $Pending -and -not $changed
    }
}

function Get-Cs2AspectGuidance {
    param([int]$Width, [int]$Height, [string]$Mode)
    if ($Height -le 0) { return $null }
    $ratio = [double]$Width / [double]$Height
    $implied = $null
    $impliedMode = $null
    if ([math]::Abs($ratio - (16.0 / 9.0)) -lt 0.015) { $implied = '16:9'; $impliedMode = '1' }
    elseif ([math]::Abs($ratio - (16.0 / 10.0)) -lt 0.015) { $implied = '16:10'; $impliedMode = '2' }
    elseif ([math]::Abs($ratio - (4.0 / 3.0)) -lt 0.015) { $implied = '4:3'; $impliedMode = '0' }
    elseif ([math]::Abs($ratio - (5.0 / 4.0)) -lt 0.015) { $implied = '5:4'; $impliedMode = '0' }
    if ($implied -and $impliedMode -ne [string]$Mode) {
        return "Dimensions are $implied; aspect mode remains $(ConvertFrom-AspectMode $Mode)."
    }
    $null
}

function Show-Cs2BackupHistoryDialog {
    param(
        $Owner,
        [string]$Path,
        $Theme,
        $FontNormal,
        $FontSmall
    )
    $dialog = New-Object Windows.Forms.Form
    $dialog.Text = 'Backup history'
    $dialog.ShowIcon = $false
    $dialog.StartPosition = 'CenterParent'
    $dialog.ClientSize = New-Object Drawing.Size(720, 430)
    $dialog.MinimumSize = New-Object Drawing.Size(620, 380)
    $dialog.AutoScaleMode = 'Dpi'
    $dialog.BackColor = $Theme.Background
    $dialog.ForeColor = $Theme.Text
    $dialog.Font = $FontNormal

    $layout = New-Object Windows.Forms.TableLayoutPanel
    $layout.Dock = 'Fill'; $layout.ColumnCount = 1; $layout.RowCount = 5; $layout.Padding = New-Object Windows.Forms.Padding(20)
    [void]$layout.RowStyles.Add((New-Object Windows.Forms.RowStyle('Absolute', 34)))
    [void]$layout.RowStyles.Add((New-Object Windows.Forms.RowStyle('Percent', 100)))
    [void]$layout.RowStyles.Add((New-Object Windows.Forms.RowStyle('Absolute', 48)))
    [void]$layout.RowStyles.Add((New-Object Windows.Forms.RowStyle('Absolute', 40)))
    [void]$layout.RowStyles.Add((New-Object Windows.Forms.RowStyle('Absolute', 48)))
    $dialog.Controls.Add($layout)

    $heading = New-Object Windows.Forms.Label
    $heading.Text = 'Select a backup to preview and restore.'; $heading.Dock = 'Fill'; $heading.ForeColor = $Theme.Muted
    $heading.AccessibleName = $heading.Text
    $layout.Controls.Add($heading, 0, 0)

    $list = New-Object Windows.Forms.ListView
    $list.Dock = 'Fill'; $list.View = 'Details'; $list.FullRowSelect = $true; $list.MultiSelect = $false
    $list.HideSelection = $false; $list.BackColor = $Theme.Input; $list.ForeColor = $Theme.Text; $list.BorderStyle = 'FixedSingle'
    [void]$list.Columns.Add('Date', 142); [void]$list.Columns.Add('Resolution', 110)
    [void]$list.Columns.Add('Aspect', 105); [void]$list.Columns.Add('File', 300)
    $list.AccessibleName = 'Configuration backup history'
    $layout.Controls.Add($list, 0, 1)

    $preview = New-Object Windows.Forms.Label
    $preview.Dock = 'Fill'; $preview.Padding = New-Object Windows.Forms.Padding(12, 0, 12, 0)
    $preview.BackColor = $Theme.Card; $preview.ForeColor = $Theme.Muted; $preview.TextAlign = 'MiddleLeft'
    $preview.Text = 'Select a valid backup to compare it with the active configuration.'
    $preview.AccessibleName = "Restore preview: $($preview.Text)"
    $layout.Controls.Add($preview, 0, 2)

    $warning = New-Object Windows.Forms.Label
    $warning.Dock = 'Fill'; $warning.ForeColor = $Theme.WarningText; $warning.TextAlign = 'MiddleLeft'
    $warning.Text = 'Restoring creates a rollback backup of the active file.'
    $layout.Controls.Add($warning, 0, 3)

    $buttons = New-Object Windows.Forms.FlowLayoutPanel
    $buttons.Dock = 'Fill'; $buttons.FlowDirection = 'RightToLeft'; $buttons.WrapContents = $false
    $restore = New-Object Windows.Forms.Button
    $restore.Text = 'Restore backup'; $restore.Width = 132; $restore.Height = 38; $restore.Enabled = $false
    $restore.FlatStyle = 'Flat'; $restore.FlatAppearance.BorderSize = 0; $restore.BackColor = $Theme.Border; $restore.ForeColor = $Theme.Muted
    $restore.AccessibleName = 'Restore selected configuration backup'
    $close = New-Object Windows.Forms.Button
    $close.Text = 'Close'; $close.Width = 88; $close.Height = 38; $close.DialogResult = 'Cancel'
    $close.FlatStyle = 'Flat'; $close.FlatAppearance.BorderColor = $Theme.Border; $close.BackColor = $Theme.Input; $close.ForeColor = $Theme.Text
    $buttons.Controls.Add($restore); $buttons.Controls.Add($close)
    $layout.Controls.Add($buttons, 0, 4)
    $dialog.CancelButton = $close

    $backups = @(Get-Cs2BackupFiles $Path)
    $current = Get-CurrentConfig $Path
    foreach ($backup in $backups) {
        $dateText = if ($backup.Timestamp -eq [datetime]::MinValue) { 'Unknown' } else { $backup.Timestamp.ToString('yyyy-MM-dd HH:mm:ss') }
        $resolutionText = if ($backup.Valid) { "$($backup.Config.Width)x$($backup.Config.Height)" } else { 'Invalid' }
        $aspectText = if ($backup.Valid) { ConvertFrom-AspectMode $backup.Config.Mode } else { '-' }
        $item = New-Object Windows.Forms.ListViewItem($dateText)
        [void]$item.SubItems.Add($resolutionText); [void]$item.SubItems.Add($aspectText); [void]$item.SubItems.Add($backup.Name)
        $item.Tag = $backup
        if (-not $backup.Valid) { $item.ForeColor = $Theme.ErrorText }
        [void]$list.Items.Add($item)
    }
    if ($backups.Count -eq 0) {
        $preview.Text = 'No editor-created backups were found for this configuration.'
        $preview.AccessibleName = "Restore preview: $($preview.Text)"
    }

    $list.Add_SelectedIndexChanged({
        $restore.Enabled = $false
        $restore.BackColor = $Theme.Border; $restore.ForeColor = $Theme.Muted
        if ($list.SelectedItems.Count -ne 1) { return }
        $selected = $list.SelectedItems[0].Tag
        if (-not $selected.Valid) {
            $preview.Text = "Invalid backup: $($selected.Error)"
            $preview.ForeColor = $Theme.ErrorText
        } else {
            $preview.Text = "Current $($current.Width)x$($current.Height) $(ConvertFrom-AspectMode $current.Mode)  >  Restore $($selected.Config.Width)x$($selected.Config.Height) $(ConvertFrom-AspectMode $selected.Config.Mode)"
            $preview.ForeColor = $Theme.Text
            $restore.Enabled = -not (Test-Cs2ConfigEquals $current $selected.Config)
            if ($restore.Enabled) { $restore.BackColor = $Theme.Primary; $restore.ForeColor = $Theme.PrimaryText }
        }
        $preview.AccessibleName = "Restore preview: $($preview.Text)"
    })

    $restore.Add_Click({
        if ($list.SelectedItems.Count -ne 1) { return }
        $selected = $list.SelectedItems[0].Tag
        $message = "Restore $($selected.Config.Width)x$($selected.Config.Height) ($(ConvertFrom-AspectMode $selected.Config.Mode))?`r`n`r`nA rollback backup of the active file will be created."
        if ([Windows.Forms.MessageBox]::Show($message, 'Restore configuration backup', 'YesNo', 'Warning') -ne 'Yes') { return }
        if (Get-Process -Name 'cs2' -ErrorAction SilentlyContinue) {
            $runningAnswer = [Windows.Forms.MessageBox]::Show('CS2 is running and may overwrite the restored configuration. Restore anyway?', 'CS2 is running', 'YesNo', 'Warning')
            if ($runningAnswer -ne 'Yes') { return }
        }
        try {
            $dialog.Tag = Restore-Cs2Backup $Path $selected.Path
            $dialog.DialogResult = 'OK'
            $dialog.Close()
        } catch {
            [void][Windows.Forms.MessageBox]::Show($_.Exception.Message, 'Restore failed', 'OK', 'Error')
        }
    })

    try {
        [void]$dialog.ShowDialog($Owner)
        $dialog.Tag
    } finally {
        $dialog.Dispose()
    }
}

function New-Cs2UiCard {
    param([psobject]$Theme)
    $card = New-Object Windows.Forms.Panel
    $card.Dock = 'Fill'
    $card.BackColor = $Theme.Card
    $card.ForeColor = $Theme.Text
    $card.Padding = New-Object Windows.Forms.Padding(18, 14, 18, 14)
    $card.Margin = New-Object Windows.Forms.Padding(0)
    $card.Add_Resize({ param($sender, $eventArgs) Set-Cs2UiRoundedRegion $sender 12 })
    $paintHandler = {
        param($sender, $eventArgs)
        $eventArgs.Graphics.SmoothingMode = [Drawing.Drawing2D.SmoothingMode]::AntiAlias
        $bounds = New-Object Drawing.Rectangle(0, 0, ($sender.Width - 1), ($sender.Height - 1))
        $path = New-Cs2UiRoundedPath $bounds 12
        $pen = New-Object Drawing.Pen($Theme.Border, 1)
        try {
            $eventArgs.Graphics.DrawPath($pen, $path)
        } finally {
            $pen.Dispose()
            $path.Dispose()
        }
    }.GetNewClosure()
    $card.Add_Paint($paintHandler)
    $card
}

function Set-Cs2UiInputStyle {
    param([Windows.Forms.Control]$Control, [psobject]$Theme, [Drawing.Font]$Font)
    $Control.BackColor = $Theme.Input
    $Control.ForeColor = $Theme.Text
    $Control.Font = $Font
    if ($Control -is [Windows.Forms.ComboBox]) { $Control.FlatStyle = 'Flat' }
}

function Set-Cs2UiSecondaryButtonStyle {
    param([Windows.Forms.Button]$Button, [psobject]$Theme)
    $Button.FlatStyle = 'Flat'
    $Button.FlatAppearance.BorderColor = $Theme.Border
    $Button.FlatAppearance.MouseOverBackColor = $Theme.CardHover
    $Button.FlatAppearance.MouseDownBackColor = $Theme.Border
    $Button.BackColor = $Theme.Input
    $Button.ForeColor = $Theme.Text
    $Button.UseVisualStyleBackColor = $false
}

function Get-Cs2ResponsiveBreakpoint {
    param([int]$Dpi)
    if ($Dpi -lt 48 -or $Dpi -gt 768) { throw "Unsupported DPI value '$Dpi'." }
    [int](900 * ([double]$Dpi / 96.0))
}

function Get-Cs2DisplayPreviewBounds {
    param(
        [int]$CanvasWidth,
        [int]$CanvasHeight,
        [int]$Width,
        [int]$Height,
        [int]$Padding = 10
    )
    if ($CanvasWidth -le ($Padding * 2) -or $CanvasHeight -le ($Padding * 2) -or
        $Width -le 0 -or $Height -le 0) {
        return New-Object Drawing.Rectangle(0, 0, 0, 0)
    }
    $availableWidth = $CanvasWidth - ($Padding * 2)
    $availableHeight = $CanvasHeight - ($Padding * 2)
    $scale = [math]::Min($availableWidth / [double]$Width, $availableHeight / [double]$Height)
    $previewWidth = [math]::Max(1, [int][math]::Round($Width * $scale))
    $previewHeight = [math]::Max(1, [int][math]::Round($Height * $scale))
    New-Object Drawing.Rectangle(
        [int](($CanvasWidth - $previewWidth) / 2),
        [int](($CanvasHeight - $previewHeight) / 2),
        $previewWidth,
        $previewHeight
    )
}

function Set-Cs2ResponsiveEditorLayout {
    param(
        [Windows.Forms.Form]$Form,
        [Windows.Forms.TableLayoutPanel]$Body,
        [Windows.Forms.Control]$AccountCard,
        [Windows.Forms.Control]$DisplayCard,
        [Windows.Forms.Control]$StatusLabel,
        [psobject]$UiState
    )
    $threshold = Get-Cs2ResponsiveBreakpoint $Form.DeviceDpi
    $wide = $Form.ClientSize.Width -ge $threshold
    if ($wide -eq $UiState.IsWide -and $Body.ColumnStyles.Count -gt 0) { return }
    $focused = $Form.ActiveControl
    $Body.SuspendLayout()
    try {
        $Body.ColumnStyles.Clear()
        $Body.RowStyles.Clear()
        if ($wide) {
            $Body.ColumnCount = 2
            $Body.RowCount = 2
            [void]$Body.ColumnStyles.Add((New-Object Windows.Forms.ColumnStyle('Percent', 42)))
            [void]$Body.ColumnStyles.Add((New-Object Windows.Forms.ColumnStyle('Percent', 58)))
            [void]$Body.RowStyles.Add((New-Object Windows.Forms.RowStyle('Percent', 100)))
            [void]$Body.RowStyles.Add((New-Object Windows.Forms.RowStyle('Absolute', 48)))
            $Body.SetCellPosition($AccountCard, (New-Object Windows.Forms.TableLayoutPanelCellPosition(0, 0)))
            $Body.SetCellPosition($DisplayCard, (New-Object Windows.Forms.TableLayoutPanelCellPosition(1, 0)))
            $Body.SetCellPosition($StatusLabel, (New-Object Windows.Forms.TableLayoutPanelCellPosition(0, 1)))
            $Body.SetColumnSpan($StatusLabel, 2)
            $AccountCard.Margin = New-Object Windows.Forms.Padding(0, 0, 6, 8)
            $DisplayCard.Margin = New-Object Windows.Forms.Padding(6, 0, 0, 8)
        } else {
            $Body.ColumnCount = 1
            $Body.RowCount = 3
            [void]$Body.ColumnStyles.Add((New-Object Windows.Forms.ColumnStyle('Percent', 100)))
            [void]$Body.RowStyles.Add((New-Object Windows.Forms.RowStyle('Absolute', 228)))
            [void]$Body.RowStyles.Add((New-Object Windows.Forms.RowStyle('Absolute', 420)))
            [void]$Body.RowStyles.Add((New-Object Windows.Forms.RowStyle('Absolute', 48)))
            $Body.SetCellPosition($AccountCard, (New-Object Windows.Forms.TableLayoutPanelCellPosition(0, 0)))
            $Body.SetCellPosition($DisplayCard, (New-Object Windows.Forms.TableLayoutPanelCellPosition(0, 1)))
            $Body.SetCellPosition($StatusLabel, (New-Object Windows.Forms.TableLayoutPanelCellPosition(0, 2)))
            $Body.SetColumnSpan($StatusLabel, 1)
            $AccountCard.Margin = New-Object Windows.Forms.Padding(0, 0, 0, 8)
            $DisplayCard.Margin = New-Object Windows.Forms.Padding(0, 0, 0, 8)
        }
        $UiState.IsWide = $wide
    } finally {
        $Body.ResumeLayout($true)
        if ($focused -and $focused.CanFocus) { [void]$focused.Focus() }
    }
}

function Show-ModernGraphicalEditor {
    param(
        [object[]]$SteamAccounts,
        [string]$InitialPath,
        [psobject]$Preferences = (New-Cs2Preferences),
        [string[]]$SteamRoots = @(),
        [string]$SettingsPath = (Get-Cs2SettingsPath)
    )

    Initialize-Cs2GraphicalInterface
    $highContrast = [Windows.Forms.SystemInformation]::HighContrast
    $theme = Get-Cs2UiTheme $highContrast

    $installedFonts = New-Object Drawing.Text.InstalledFontCollection
    try {
        $fontFamily = if ($installedFonts.Families.Name -contains 'Segoe UI Variable') { 'Segoe UI Variable' } else { 'Segoe UI' }
    } finally {
        $installedFonts.Dispose()
    }
    $fontNormal = New-Object Drawing.Font($fontFamily, 10)
    $fontSmall = New-Object Drawing.Font($fontFamily, 8.5)
    $fontSection = New-Object Drawing.Font($fontFamily, 11, [Drawing.FontStyle]::Bold)
    $fontPreview = New-Object Drawing.Font($fontFamily, 12, [Drawing.FontStyle]::Bold)
    $fontTitle = New-Object Drawing.Font($fontFamily, 19, [Drawing.FontStyle]::Bold)

    $form = $null
    $statusToolTip = $null
    $accountToolTip = $null
    try {
    $form = New-Object Windows.Forms.Form
    $form.Text = 'CS2 Video Configuration'
    $form.AccessibleDescription = "CS2 Video Config Editor version $script:ApplicationVersion"
    $form.StartPosition = 'CenterScreen'
    $form.ShowIcon = $false
    $form.MaximizeBox = $true
    $form.ClientSize = New-Object Drawing.Size(980, 650)
    $form.MinimumSize = New-Object Drawing.Size(700, 560)
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
    $body.Dock = 'Fill'; $body.ColumnCount = 2; $body.RowCount = 2; $body.Padding = New-Object Windows.Forms.Padding(20, 16, 20, 12); $body.AutoScroll = $true
    [void]$body.ColumnStyles.Add((New-Object Windows.Forms.ColumnStyle('Percent', 42)))
    [void]$body.ColumnStyles.Add((New-Object Windows.Forms.ColumnStyle('Percent', 58)))
    [void]$body.RowStyles.Add((New-Object Windows.Forms.RowStyle('Percent', 100)))
    [void]$body.RowStyles.Add((New-Object Windows.Forms.RowStyle('Absolute', 48)))
    $root.Controls.Add($body, 0, 1)

    $accountCard = New-Cs2UiCard $theme
    $accountCard.Margin = New-Object Windows.Forms.Padding(0, 0, 6, 8)
    $accountCard.TabIndex = 0
    $body.Controls.Add($accountCard, 0, 0)
    $accountLayout = New-Object Windows.Forms.TableLayoutPanel
    $accountLayout.Dock = 'Top'; $accountLayout.Height = 180; $accountLayout.ColumnCount = 2; $accountLayout.RowCount = 5; $accountLayout.Margin = New-Object Windows.Forms.Padding(0)
    $accountLayout.TabIndex = 0
    [void]$accountLayout.ColumnStyles.Add((New-Object Windows.Forms.ColumnStyle('Percent', 100)))
    [void]$accountLayout.ColumnStyles.Add((New-Object Windows.Forms.ColumnStyle('Absolute', 100)))
    foreach ($height in @(32, 48, 20, 40, 36)) { [void]$accountLayout.RowStyles.Add((New-Object Windows.Forms.RowStyle('Absolute', $height))) }
    $accountCard.Controls.Add($accountLayout)

    $accountTitle = New-Object Windows.Forms.Label
    $accountTitle.Text = 'Steam account'; $accountTitle.Font = $fontSection; $accountTitle.ForeColor = $theme.Text; $accountTitle.Dock = 'Fill'; $accountTitle.TextAlign = 'MiddleLeft'
    $accountLayout.Controls.Add($accountTitle, 0, 0)
    $refreshAccounts = New-Object Windows.Forms.Button
    $refreshAccounts.Text = 'Refresh'; $refreshAccounts.Dock = 'Fill'; $refreshAccounts.Margin = New-Object Windows.Forms.Padding(0, 0, 0, 2)
    Set-Cs2UiSecondaryButtonStyle $refreshAccounts $theme
    $refreshAccounts.AccessibleName = 'Refresh Steam accounts'
    $accountLayout.Controls.Add($refreshAccounts, 1, 0)

    $accountBox = New-Object Windows.Forms.ComboBox
    $accountBox.Dock = 'Fill'; $accountBox.DropDownStyle = 'DropDownList'; $accountBox.DisplayMember = 'DisplayName'; $accountBox.DrawMode = 'OwnerDrawFixed'
    $accountBox.ItemHeight = 38; $accountBox.IntegralHeight = $false; $accountBox.DropDownHeight = 240; $accountBox.DropDownWidth = 690
    Set-Cs2UiInputStyle $accountBox $theme $fontNormal
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
        try {
            $eventArgs.Graphics.FillRectangle($brush, $eventArgs.Bounds)
        } finally {
            $brush.Dispose()
        }
        $primaryBrush = New-Object Drawing.SolidBrush($fore)
        $secondaryBrush = New-Object Drawing.SolidBrush($muted)
        $format = New-Object Drawing.StringFormat
        try {
            $format.Trimming = [Drawing.StringTrimming]::EllipsisCharacter
            $format.FormatFlags = [Drawing.StringFormatFlags]::NoWrap
            $primaryBounds = New-Object Drawing.RectangleF(($eventArgs.Bounds.X + 10), ($eventArgs.Bounds.Y + 3), ($eventArgs.Bounds.Width - 20), 18)
            $secondaryBounds = New-Object Drawing.RectangleF(($eventArgs.Bounds.X + 10), ($eventArgs.Bounds.Y + 21), ($eventArgs.Bounds.Width - 20), 15)
            $eventArgs.Graphics.DrawString([string]$account.PersonaName, $fontNormal, $primaryBrush, $primaryBounds, $format)
            $details = Format-SteamAccountIdentifiers $account
            $eventArgs.Graphics.DrawString($details, $fontSmall, $secondaryBrush, $secondaryBounds, $format)
        } finally {
            $primaryBrush.Dispose()
            $secondaryBrush.Dispose()
            $format.Dispose()
        }
        $eventArgs.DrawFocusRectangle()
    })
    $accountBox.Add_Resize({ param($sender, $eventArgs) $sender.DropDownWidth = [math]::Max(420, $sender.Width) })
    $accountLayout.Controls.Add($accountBox, 0, 1); $accountLayout.SetColumnSpan($accountBox, 2)

    $pathLabel = New-Object Windows.Forms.Label
    $pathLabel.Text = 'Configuration file'; $pathLabel.Font = $fontSmall; $pathLabel.ForeColor = $theme.Muted; $pathLabel.Dock = 'Fill'; $pathLabel.TextAlign = 'BottomLeft'
    $accountLayout.Controls.Add($pathLabel, 0, 2); $accountLayout.SetColumnSpan($pathLabel, 2)
    $pathBox = New-Object Windows.Forms.TextBox
    $pathBox.Dock = 'Fill'; $pathBox.BorderStyle = 'FixedSingle'; $pathBox.Margin = New-Object Windows.Forms.Padding(0, 3, 8, 3)
    Set-Cs2UiInputStyle $pathBox $theme $fontNormal
    [void]$darkNativeControls.Add($pathBox)
    if ($InitialPath) { $pathBox.Text = $InitialPath }
    $accountLayout.Controls.Add($pathBox, 0, 3)
    $browse = New-Object Windows.Forms.Button
    $browse.Text = 'Browse...'; $browse.Dock = 'Fill'; $browse.Margin = New-Object Windows.Forms.Padding(0, 3, 0, 3)
    Set-Cs2UiSecondaryButtonStyle $browse $theme
    $accountLayout.Controls.Add($browse, 1, 3)

    $currentLabel = New-Object Windows.Forms.Label
    $currentLabel.Text = 'Checking configuration...'; $currentLabel.Dock = 'Fill'; $currentLabel.Margin = New-Object Windows.Forms.Padding(0, 4, 8, 0)
    $currentLabel.Padding = New-Object Windows.Forms.Padding(10, 0, 10, 0); $currentLabel.TextAlign = 'MiddleLeft'; $currentLabel.BackColor = $theme.CardHover; $currentLabel.ForeColor = $theme.Muted
    $currentLabel.Add_Resize({ param($sender, $eventArgs) Set-Cs2UiRoundedRegion $sender 7 })
    $accountLayout.Controls.Add($currentLabel, 0, 4)
    $backupsButton = New-Object Windows.Forms.Button
    $backupsButton.Text = 'Backups...'; $backupsButton.Dock = 'Fill'; $backupsButton.Margin = New-Object Windows.Forms.Padding(0, 4, 0, 0)
    Set-Cs2UiSecondaryButtonStyle $backupsButton $theme
    $backupsButton.AccessibleName = 'Open configuration backup history'
    $backupsButton.Enabled = $false
    $accountLayout.Controls.Add($backupsButton, 1, 4)

    $displayCard = New-Cs2UiCard $theme
    $displayCard.Margin = New-Object Windows.Forms.Padding(6, 0, 0, 8)
    $displayCard.TabIndex = 1
    $body.Controls.Add($displayCard, 1, 0)
    $displayLayout = New-Object Windows.Forms.TableLayoutPanel
    $displayLayout.Dock = 'Fill'; $displayLayout.ColumnCount = 2; $displayLayout.RowCount = 8; $displayLayout.Margin = New-Object Windows.Forms.Padding(0)
    $displayLayout.TabIndex = 0
    [void]$displayLayout.ColumnStyles.Add((New-Object Windows.Forms.ColumnStyle('Absolute', 128)))
    [void]$displayLayout.ColumnStyles.Add((New-Object Windows.Forms.ColumnStyle('Percent', 100)))
    foreach ($height in @(32, 42, 42, 42, 18, 74, 92, 42)) { [void]$displayLayout.RowStyles.Add((New-Object Windows.Forms.RowStyle('Absolute', $height))) }
    $displayCard.Controls.Add($displayLayout)

    $displayTitle = New-Object Windows.Forms.Label
    $displayTitle.Text = 'Display settings'; $displayTitle.Font = $fontSection; $displayTitle.ForeColor = $theme.Text; $displayTitle.Dock = 'Fill'; $displayTitle.TextAlign = 'MiddleLeft'
    $displayLayout.Controls.Add($displayTitle, 0, 0)
    $reset = New-Object Windows.Forms.Button
    $reset.Text = 'Reset'; $reset.Width = 78; $reset.Height = 30; $reset.Anchor = 'Top,Right'; $reset.Margin = New-Object Windows.Forms.Padding(0)
    $reset.FlatStyle = 'Flat'; $reset.FlatAppearance.BorderColor = $theme.Border; $reset.FlatAppearance.MouseOverBackColor = $theme.CardHover
    $reset.BackColor = $theme.Input; $reset.ForeColor = $theme.Text; $reset.UseVisualStyleBackColor = $false; $reset.Enabled = $false
    $reset.AccessibleName = 'Reset pending display settings'
    $reset.TabIndex = 3
    $reset.Add_EnabledChanged({
        if ($reset.Enabled) { $reset.BackColor = $theme.Input; $reset.ForeColor = $theme.Text }
        else { $reset.BackColor = $theme.Card; $reset.ForeColor = $theme.Muted }
    })
    $displayLayout.Controls.Add($reset, 1, 0)

    $aspectLabel = New-Object Windows.Forms.Label
    $aspectLabel.Text = 'Aspect ratio'; $aspectLabel.ForeColor = $theme.Muted; $aspectLabel.Dock = 'Fill'; $aspectLabel.TextAlign = 'MiddleLeft'
    $displayLayout.Controls.Add($aspectLabel, 0, 1)
    $aspectPanel = New-Object Windows.Forms.TableLayoutPanel
    $aspectPanel.Dock = 'Fill'; $aspectPanel.ColumnCount = 3; $aspectPanel.RowCount = 1; $aspectPanel.Margin = New-Object Windows.Forms.Padding(0, 3, 0, 3)
    $aspectPanel.TabIndex = 0
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
                $button.AccessibleDescription = "$($button.Text) aspect mode is selected. Use the left and right arrow keys to change it."
            } else {
                $button.BackColor = $theme.Input; $button.ForeColor = $theme.Text; $button.FlatAppearance.BorderColor = $theme.Border
                $button.AccessibleDescription = "$($button.Text) aspect mode is not selected. Use the left and right arrow keys to select it."
            }
        }
    }
    foreach ($definition in $ratioDefinitions) {
        $radio = New-Object Windows.Forms.RadioButton
        $radio.Text = $definition.Text; $radio.Tag = $definition.Mode; $radio.Appearance = 'Button'; $radio.Dock = 'Fill'; $radio.TextAlign = 'MiddleCenter'
        $radio.FlatStyle = 'Flat'; $radio.FlatAppearance.BorderSize = 1; $radio.FlatAppearance.MouseOverBackColor = $theme.CardHover; $radio.FlatAppearance.MouseDownBackColor = $theme.Border; $radio.Margin = New-Object Windows.Forms.Padding(0, 0, 6, 0); $radio.UseVisualStyleBackColor = $false
        $radio.AccessibleName = "Aspect ratio $($definition.Text)"
        $radio.TabIndex = [int]$definition.Mode
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
    $resolutionBox.TabIndex = 1
    Set-Cs2UiInputStyle $resolutionBox $theme $fontNormal
    [void]$darkNativeControls.Add($resolutionBox)
    $displayLayout.Controls.Add($resolutionBox, 1, 2)

    $customLabel = New-Object Windows.Forms.Label
    $customLabel.Text = 'Custom size'; $customLabel.ForeColor = $theme.Muted; $customLabel.Dock = 'Fill'; $customLabel.TextAlign = 'MiddleLeft'
    $displayLayout.Controls.Add($customLabel, 0, 3)
    $customPanel = New-Object Windows.Forms.FlowLayoutPanel
    $customPanel.Dock = 'Fill'; $customPanel.FlowDirection = 'LeftToRight'; $customPanel.WrapContents = $false; $customPanel.Margin = New-Object Windows.Forms.Padding(0, 4, 0, 2)
    $customPanel.TabIndex = 2
    $widthBox = New-Object Windows.Forms.NumericUpDown
    $widthBox.Minimum = 320; $widthBox.Maximum = 32768; $widthBox.Value = 1920; $widthBox.Width = 130; $widthBox.BorderStyle = 'FixedSingle'
    $widthBox.TabIndex = 0
    Set-Cs2UiInputStyle $widthBox $theme $fontNormal
    [void]$darkNativeControls.Add($widthBox)
    $timesLabel = New-Object Windows.Forms.Label
    $timesLabel.Text = ' x '; $timesLabel.ForeColor = $theme.Muted; $timesLabel.AutoSize = $true; $timesLabel.Padding = New-Object Windows.Forms.Padding(8, 5, 8, 0)
    $heightBox = New-Object Windows.Forms.NumericUpDown
    $heightBox.Minimum = 200; $heightBox.Maximum = 32768; $heightBox.Value = 1080; $heightBox.Width = 130; $heightBox.BorderStyle = 'FixedSingle'
    $heightBox.TabIndex = 1
    Set-Cs2UiInputStyle $heightBox $theme $fontNormal
    [void]$darkNativeControls.Add($heightBox)
    $customPanel.Controls.Add($widthBox); $customPanel.Controls.Add($timesLabel); $customPanel.Controls.Add($heightBox)
    $displayLayout.Controls.Add($customPanel, 1, 3)
    $customHint = New-Object Windows.Forms.Label
    $customHint.Text = 'Choose Custom... to enter an exact width and height.'; $customHint.Dock = 'Fill'; $customHint.TextAlign = 'MiddleLeft'; $customHint.ForeColor = $theme.Muted
    $displayLayout.Controls.Add($customHint, 1, 3)

    $previewHeading = New-Object Windows.Forms.Label
    $previewHeading.Text = 'CURRENT AND PENDING'; $previewHeading.Font = $fontSmall; $previewHeading.ForeColor = $theme.Muted
    $previewHeading.Dock = 'Fill'; $previewHeading.TextAlign = 'BottomLeft'
    $displayLayout.Controls.Add($previewHeading, 0, 4); $displayLayout.SetColumnSpan($previewHeading, 2)

    $previewPanel = New-Object Windows.Forms.TableLayoutPanel
    $previewPanel.Dock = 'Fill'; $previewPanel.ColumnCount = 3; $previewPanel.RowCount = 2; $previewPanel.Margin = New-Object Windows.Forms.Padding(0, 5, 0, 5)
    [void]$previewPanel.ColumnStyles.Add((New-Object Windows.Forms.ColumnStyle('Percent', 45)))
    [void]$previewPanel.ColumnStyles.Add((New-Object Windows.Forms.ColumnStyle('Percent', 10)))
    [void]$previewPanel.ColumnStyles.Add((New-Object Windows.Forms.ColumnStyle('Percent', 45)))
    [void]$previewPanel.RowStyles.Add((New-Object Windows.Forms.RowStyle('Absolute', 18)))
    [void]$previewPanel.RowStyles.Add((New-Object Windows.Forms.RowStyle('Percent', 100)))
    $displayLayout.Controls.Add($previewPanel, 0, 5); $displayLayout.SetColumnSpan($previewPanel, 2)
    $currentCaption = New-Object Windows.Forms.Label
    $currentCaption.Text = 'CURRENT'; $currentCaption.Font = $fontSmall; $currentCaption.ForeColor = $theme.Muted; $currentCaption.Dock = 'Fill'; $currentCaption.TextAlign = 'MiddleLeft'
    $pendingCaption = New-Object Windows.Forms.Label
    $pendingCaption.Text = 'PENDING'; $pendingCaption.Font = $fontSmall; $pendingCaption.ForeColor = $theme.Muted; $pendingCaption.Dock = 'Fill'; $pendingCaption.TextAlign = 'MiddleLeft'
    $previewArrow = New-Object Windows.Forms.Label
    $previewArrow.Text = '>'; $previewArrow.ForeColor = $theme.Muted; $previewArrow.Dock = 'Fill'; $previewArrow.TextAlign = 'MiddleCenter'
    $currentPreview = New-Object Windows.Forms.Label
    $currentPreview.Text = '-'; $currentPreview.Font = $fontPreview; $currentPreview.ForeColor = $theme.Text; $currentPreview.Dock = 'Fill'; $currentPreview.TextAlign = 'MiddleLeft'
    $pendingPreview = New-Object Windows.Forms.Label
    $pendingPreview.Text = '-'; $pendingPreview.Font = $fontPreview; $pendingPreview.ForeColor = $theme.Text; $pendingPreview.Dock = 'Fill'; $pendingPreview.TextAlign = 'MiddleLeft'
    $previewPanel.Controls.Add($currentCaption, 0, 0); $previewPanel.Controls.Add($pendingCaption, 2, 0)
    $previewPanel.Controls.Add($currentPreview, 0, 1); $previewPanel.Controls.Add($previewArrow, 1, 1); $previewPanel.Controls.Add($pendingPreview, 2, 1)

    $displayPreview = New-Object Windows.Forms.Panel
    $displayPreview.Dock = 'Fill'; $displayPreview.Margin = New-Object Windows.Forms.Padding(0, 4, 0, 4)
    $displayPreview.BackColor = $theme.Input
    $displayPreview.AccessibleName = 'Pending display shape preview'
    $displayPreview.AccessibleDescription = 'A visual preview of the selected resolution and aspect ratio.'
    $displayPreview.Tag = [pscustomobject]@{ Width = 1920; Height = 1080; Label = '1920x1080  16:9' }
    $displayPreview.Add_Paint({
        param($sender, $eventArgs)
        $eventArgs.Graphics.SmoothingMode = [Drawing.Drawing2D.SmoothingMode]::AntiAlias
        $data = $sender.Tag
        if ($null -eq $data) { return }
        $bounds = Get-Cs2DisplayPreviewBounds $sender.ClientSize.Width $sender.ClientSize.Height $data.Width $data.Height 12
        if ($bounds.Width -le 0 -or $bounds.Height -le 0) { return }
        $screenBrush = New-Object Drawing.SolidBrush($theme.CardHover)
        $borderPen = New-Object Drawing.Pen($theme.Primary, 2)
        $labelBrush = New-Object Drawing.SolidBrush($theme.Text)
        $format = New-Object Drawing.StringFormat
        try {
            $format.Alignment = [Drawing.StringAlignment]::Center
            $format.LineAlignment = [Drawing.StringAlignment]::Center
            $eventArgs.Graphics.FillRectangle($screenBrush, $bounds)
            $eventArgs.Graphics.DrawRectangle($borderPen, $bounds)
            $eventArgs.Graphics.DrawString(
                [string]$data.Label,
                $fontSmall,
                $labelBrush,
                (New-Object Drawing.RectangleF($bounds.X, $bounds.Y, $bounds.Width, $bounds.Height)),
                $format
            )
        } finally {
            $screenBrush.Dispose(); $borderPen.Dispose(); $labelBrush.Dispose(); $format.Dispose()
        }
    })
    $displayPreview.Add_Resize({ param($sender, $eventArgs) $sender.Invalidate() })
    $displayLayout.Controls.Add($displayPreview, 0, 6); $displayLayout.SetColumnSpan($displayPreview, 2)

    $mismatchLabel = New-Object Windows.Forms.Label
    $mismatchLabel.Dock = 'Fill'; $mismatchLabel.ForeColor = $theme.Muted; $mismatchLabel.TextAlign = 'MiddleLeft'
    $mismatchLabel.AutoEllipsis = $true; $mismatchLabel.Visible = $false
    $displayLayout.Controls.Add($mismatchLabel, 0, 7); $displayLayout.SetColumnSpan($mismatchLabel, 2)

    $statusLabel = New-Object Windows.Forms.Label
    $statusLabel.Dock = 'Fill'; $statusLabel.Margin = New-Object Windows.Forms.Padding(0); $statusLabel.Padding = New-Object Windows.Forms.Padding(14, 0, 14, 0)
    $statusLabel.TextAlign = 'MiddleLeft'; $statusLabel.BackColor = $theme.CardHover; $statusLabel.ForeColor = $theme.Muted; $statusLabel.AutoEllipsis = $true
    $statusLabel.Add_Resize({ param($sender, $eventArgs) Set-Cs2UiRoundedRegion $sender 8 })
    $statusLabel.Margin = New-Object Windows.Forms.Padding(0)
    $body.Controls.Add($statusLabel, 0, 1); $body.SetColumnSpan($statusLabel, 2)
    Set-Cs2UiRoundedRegion $statusLabel 8
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
    $footer.TabIndex = 2
    [void]$footer.ColumnStyles.Add((New-Object Windows.Forms.ColumnStyle('Percent', 100)))
    [void]$footer.ColumnStyles.Add((New-Object Windows.Forms.ColumnStyle('Absolute', 238)))
    [void]$footer.RowStyles.Add((New-Object Windows.Forms.RowStyle('Percent', 100)))
    $root.Controls.Add($footer, 0, 2)
    $backupCheck = New-Object Windows.Forms.CheckBox
    $backupCheck.Text = 'Create a timestamped backup'; $backupCheck.Checked = -not $NoBackup; $backupCheck.Dock = 'Fill'; $backupCheck.ForeColor = $theme.Text
    $backupCheck.TabIndex = 0
    $backupCheck.BackColor = $theme.Card; $backupCheck.FlatStyle = 'Flat'; $backupCheck.UseVisualStyleBackColor = $false; $backupCheck.AutoSize = $false
    $backupCheck.CheckAlign = 'MiddleLeft'; $backupCheck.TextAlign = 'MiddleLeft'
    $footer.Controls.Add($backupCheck, 0, 0)
    $buttonPanel = New-Object Windows.Forms.FlowLayoutPanel
    $buttonPanel.Dock = 'Fill'; $buttonPanel.FlowDirection = 'RightToLeft'; $buttonPanel.WrapContents = $false; $buttonPanel.Margin = New-Object Windows.Forms.Padding(0); $buttonPanel.BackColor = $theme.Card
    $buttonPanel.TabIndex = 1
    $apply = New-Object Windows.Forms.Button
    $apply.Text = 'Apply changes'; $apply.Width = 132; $apply.Height = 40; $apply.FlatStyle = 'Flat'; $apply.FlatAppearance.BorderSize = 0; $apply.UseVisualStyleBackColor = $false; $apply.Enabled = $false
    $apply.TabIndex = 0
    $cancel = New-Object Windows.Forms.Button
    $cancel.Text = 'Cancel'; $cancel.Width = 92; $cancel.Height = 40; $cancel.FlatStyle = 'Flat'; $cancel.FlatAppearance.BorderColor = $theme.Border; $cancel.FlatAppearance.MouseOverBackColor = $theme.CardHover; $cancel.FlatAppearance.MouseDownBackColor = $theme.Border; $cancel.BackColor = $theme.Input; $cancel.ForeColor = $theme.Text; $cancel.UseVisualStyleBackColor = $false; $cancel.DialogResult = 'Cancel'
    $cancel.TabIndex = 1
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
    $accountBox.TabIndex = 0
    $refreshAccounts.TabIndex = 1
    $pathBox.TabIndex = 2
    $browse.TabIndex = 3
    $backupsButton.TabIndex = 4
    $accountBox.AccessibleName = 'Steam account'; $pathBox.AccessibleName = 'CS2 configuration file'; $resolutionBox.AccessibleName = 'Resolution'
    $widthBox.AccessibleName = 'Custom width'; $heightBox.AccessibleName = 'Custom height'
    $currentPreview.AccessibleName = 'Current display settings'; $pendingPreview.AccessibleName = 'Pending display settings'
    $backupCheck.AccessibleName = 'Create a timestamped backup before applying'
    $apply.AccessibleName = 'Apply pending display settings'

    $uiState = [pscustomobject]@{
        CurrentConfig = $null
        PathValid = $false
        Syncing = $false
        IsWide = $true
        Accounts = @($SteamAccounts)
        RecentConfigPaths = @($Preferences.RecentConfigPaths)
        LastAccountId = [string]$Preferences.LastAccountId
    }

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
    $getPendingResolution = {
        $mode = & $getSelectedMode
        if ($null -eq $mode -or $null -eq $resolutionBox.SelectedItem) { return $null }
        return Resolve-SelectedResolution ([string]$resolutionBox.SelectedItem) ([int]$widthBox.Value) ([int]$heightBox.Value) $mode
    }
    $updateAccountAccessibility = {
        if ($accountBox.SelectedItem) {
            $details = Format-SteamAccountDetails $accountBox.SelectedItem
            $accountBox.AccessibleName = "Steam account: $($accountBox.SelectedItem.PersonaName)"
            $accountBox.AccessibleDescription = $details
            $accountToolTip.SetToolTip($accountBox, $details)
        } else {
            $accountBox.AccessibleName = 'Steam account'
            $accountBox.AccessibleDescription = 'No Steam account is selected.'
        }
    }
    $savePreferences = {
        try {
            [void](Save-Cs2Preferences -LastAccountId $uiState.LastAccountId -RecentConfigPaths $uiState.RecentConfigPaths -Path $SettingsPath)
        } catch {
            & $setStatus "Preferences could not be saved: $($_.Exception.Message)" 'Warning'
        }
    }
    $rebuildAccountList = {
        param([object[]]$Accounts, [string]$SelectedPath, [string]$SelectedAccountId)
        $uiState.Accounts = @(Merge-Cs2AccountChoices $Accounts $uiState.RecentConfigPaths)
        $accountBox.BeginUpdate()
        try {
            $accountBox.Items.Clear()
            foreach ($account in $uiState.Accounts) { [void]$accountBox.Items.Add($account) }
            $selected = $uiState.Accounts | Where-Object {
                ($SelectedAccountId -and $_.AccountId -eq $SelectedAccountId) -or
                ($SelectedPath -and $_.ConfigPath -eq $SelectedPath)
            } | Select-Object -First 1
            if ($selected) { $accountBox.SelectedItem = $selected }
            elseif ($accountBox.Items.Count -gt 0) { $accountBox.SelectedIndex = 0 }
        } finally { $accountBox.EndUpdate() }
        & $updateAccountAccessibility
    }
    $updatePendingState = {
        if ($uiState.Syncing) { return }
        $pending = $null
        try { $pending = & $getPendingResolution } catch { }
        if ($uiState.PathValid -and $uiState.CurrentConfig) {
            $currentMode = ConvertFrom-AspectMode $uiState.CurrentConfig.Mode
            $currentPreview.Text = "$($uiState.CurrentConfig.Width)x$($uiState.CurrentConfig.Height)  $currentMode"
            $currentPreview.AccessibleName = "Current display settings: $($currentPreview.Text)"
            $currentPreview.AccessibleDescription = "Current resolution $($uiState.CurrentConfig.Width) by $($uiState.CurrentConfig.Height), aspect mode $currentMode."
        } else {
            $currentPreview.Text = 'Unavailable'
            $currentPreview.AccessibleName = 'Current display settings: unavailable'
            $currentPreview.AccessibleDescription = 'Current settings are unavailable because the configuration is not valid.'
        }
        if ($pending) {
            $pendingMode = ConvertFrom-AspectMode $pending.Mode
            $pendingPreview.Text = "$($pending.Width)x$($pending.Height)  $pendingMode"
            $pendingPreview.AccessibleName = "Pending display settings: $($pendingPreview.Text)"
            $pendingPreview.AccessibleDescription = "Pending resolution $($pending.Width) by $($pending.Height), aspect mode $pendingMode."
            $displayPreview.Tag = [pscustomobject]@{
                Width = $pending.Width
                Height = $pending.Height
                Label = "$($pending.Width)x$($pending.Height)  $pendingMode"
            }
            $displayPreview.AccessibleDescription = "Visual preview of $($pending.Width) by $($pending.Height), aspect mode $pendingMode."
            $displayPreview.Invalidate()
            $pendingState = Get-Cs2PendingState $uiState.CurrentConfig $pending $uiState.PathValid
            $apply.Enabled = $pendingState.CanApply
            $reset.Enabled = $pendingState.CanReset
            if ($pendingState.Changed) {
                $apply.Text = 'Apply changes'
                $reset.AccessibleDescription = 'Restore the pending controls to the values currently loaded from the configuration file.'
                & $setStatus 'Review the pending settings, then apply when ready.' 'Neutral'
            } elseif ($uiState.PathValid) {
                $apply.Text = 'Already active'
                $reset.AccessibleDescription = 'Reset is unavailable because the pending settings already match the current configuration.'
                & $setStatus 'The selected settings are already active.' 'Neutral'
            }
            $mismatchLabel.Visible = $false
            $mismatchLabel.Text = ''
            if ($uiState.PathValid -and $resolutionBox.SelectedItem -eq 'Custom...') {
                $guidance = Get-Cs2AspectGuidance $pending.Width $pending.Height $pending.Mode
                if ($guidance) {
                    $mismatchLabel.Text = $guidance
                    $mismatchLabel.AccessibleName = "Aspect guidance: $($mismatchLabel.Text)"
                    $mismatchLabel.Visible = $true
                }
            }
        } else {
            $pendingPreview.Text = 'Choose settings'
            $pendingPreview.AccessibleName = 'Pending display settings: choose settings'
            $pendingPreview.AccessibleDescription = 'Pending settings are not complete.'
            $displayPreview.Tag = $null
            $displayPreview.AccessibleDescription = 'No pending display settings are available to preview.'
            $displayPreview.Invalidate()
            $apply.Enabled = $false; $reset.Enabled = $false; $apply.Text = 'Apply changes'
        }
        & $updateApplyStyle
    }
    $setControlsToConfig = {
        param($config)
        if (-not $config) { return }
        $uiState.Syncing = $true
        try {
            if ($ratioButtons.ContainsKey([string]$config.Mode)) { $ratioButtons[[string]$config.Mode].Checked = $true }
            & $refreshResolutions
            $wanted = "$($config.Width)x$($config.Height)"
            if ($resolutionBox.Items.Contains($wanted)) { $resolutionBox.SelectedItem = $wanted }
            else {
                $resolutionBox.SelectedItem = 'Custom...'
                $widthBox.Value = [decimal]$config.Width; $heightBox.Value = [decimal]$config.Height
            }
        } finally { $uiState.Syncing = $false }
        & $updateRatioStyles
        & $updatePendingState
    }
    $refreshCurrent = {
        try {
            $config = Get-CurrentConfig $pathBox.Text
            $uiState.CurrentConfig = $config; $uiState.PathValid = $true
            $currentLabel.Text = "Current  |  $($config.Width)x$($config.Height)  |  $(ConvertFrom-AspectMode $config.Mode)"
            $currentLabel.BackColor = $theme.SuccessBack; $currentLabel.ForeColor = $theme.SuccessText
            $currentLabel.AccessibleName = "Valid configuration: $($currentLabel.Text)"
            $backupsButton.Enabled = $true
            & $setControlsToConfig $config
        } catch {
            $uiState.CurrentConfig = $null; $uiState.PathValid = $false
            $backupsButton.Enabled = $false
            $currentLabel.Text = if (-not (Test-Path -LiteralPath $pathBox.Text -PathType Leaf)) {
                'CS2 config not found. Choose another account or use Browse.'
            } else {
                'Configuration file is not valid or cannot be read.'
            }
            $currentLabel.BackColor = $theme.ErrorBack; $currentLabel.ForeColor = $theme.ErrorText
            $currentLabel.AccessibleName = "Invalid configuration: $($currentLabel.Text)"
            & $updatePendingState
            & $setStatus $_.Exception.Message 'Error'
        }
    }

    foreach ($radio in $ratioButtons.Values) {
        $radio.Add_CheckedChanged({
            param($sender, $eventArgs)
            & $updateRatioStyles
            if ($sender.Checked) { & $refreshResolutions; & $updatePendingState }
        })
    }
    $resolutionBox.Add_SelectedIndexChanged({
        $custom = $resolutionBox.SelectedItem -eq 'Custom...'
        $customLabel.Visible = $custom; $customPanel.Visible = $custom
        $customLabel.Enabled = $custom; $customPanel.Enabled = $custom
        $customHint.Visible = -not $custom
        & $updatePendingState
    })
    $widthBox.Add_ValueChanged($updatePendingState)
    $heightBox.Add_ValueChanged($updatePendingState)
    $reset.Add_Click({ & $setControlsToConfig $uiState.CurrentConfig })
    $accountToolTip = New-Object Windows.Forms.ToolTip
    $accountBox.Add_SelectionChangeCommitted({
        if ($accountBox.SelectedItem) {
            $pathBox.Text = $accountBox.SelectedItem.ConfigPath
            $uiState.LastAccountId = [string]$accountBox.SelectedItem.AccountId
            & $updateAccountAccessibility
            & $refreshCurrent
            & $savePreferences
        }
    })
    $pathBox.Add_TextChanged({
        $uiState.PathValid = $false; $uiState.CurrentConfig = $null
        $apply.Enabled = $false; $reset.Enabled = $false; $backupsButton.Enabled = $false
        $currentLabel.Text = 'Configuration path changed; leave the field to validate.'
        $currentLabel.BackColor = $theme.CardHover; $currentLabel.ForeColor = $theme.Muted
        $currentLabel.AccessibleName = "Validation pending: $($currentLabel.Text)"
        & $updatePendingState
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
            if ($dialog.ShowDialog($form) -eq 'OK') {
                $pathBox.Text = $dialog.FileName
                & $refreshCurrent
                if ($uiState.PathValid) {
                    $uiState.RecentConfigPaths = @(Add-Cs2RecentConfigPath $uiState.RecentConfigPaths $dialog.FileName)
                    $uiState.LastAccountId = $null
                    & $rebuildAccountList $uiState.Accounts $dialog.FileName $null
                    & $savePreferences
                }
            }
        } catch { & $setStatus $_.Exception.Message 'Error' }
        finally { if ($dialog) { $dialog.Dispose() } }
    })
    $refreshAccounts.Add_Click({
        try {
            $selectedPath = $pathBox.Text
            $selectedId = if ($accountBox.SelectedItem) { [string]$accountBox.SelectedItem.AccountId } else { $null }
            $freshAccounts = @(Get-SteamAccounts -Roots $SteamRoots)
            & $rebuildAccountList $freshAccounts $selectedPath $selectedId
            if ($accountBox.SelectedItem) {
                $pathBox.Text = $accountBox.SelectedItem.ConfigPath
                $uiState.LastAccountId = [string]$accountBox.SelectedItem.AccountId
                & $refreshCurrent
                & $savePreferences
            } else {
                $pathBox.Text = ''
                & $refreshCurrent
            }
            & $setStatus "Steam accounts refreshed: $($freshAccounts.Count) found." 'Success'
        } catch { & $setStatus "Account refresh failed: $($_.Exception.Message)" 'Error' }
    })
    $backupsButton.Add_Click({
        if (-not $uiState.PathValid) { return }
        try {
            $restoreResult = Show-Cs2BackupHistoryDialog $form $pathBox.Text $theme $fontNormal $fontSmall
            if ($restoreResult -and $restoreResult.Changed) {
                & $refreshCurrent
                $restoredResolution = New-Resolution $restoreResult.RestoredConfig.Width $restoreResult.RestoredConfig.Height $restoreResult.RestoredConfig.Mode
                $form.Tag = [pscustomobject]@{ Path = $pathBox.Text; Resolution = $restoredResolution; Result = $restoreResult }
                $restoreStatus = "Backup restored: $($restoredResolution.Display)."
                $restoreKind = 'Success'
                if ($restoreResult.RetentionWarning) {
                    $restoreStatus += " $($restoreResult.RetentionWarning)"
                    $restoreKind = 'Warning'
                }
                & $setStatus $restoreStatus $restoreKind "Rollback backup: $($restoreResult.BackupPath)"
            }
        } catch { & $setStatus "Backup history failed: $($_.Exception.Message)" 'Error' }
    })
    $apply.Add_Click({
        try {
            $path = $pathBox.Text.Trim()
            [void](Get-CurrentConfig $path)
            $mode = & $getSelectedMode
            $selected = Resolve-SelectedResolution ([string]$resolutionBox.SelectedItem) ([int]$widthBox.Value) ([int]$heightBox.Value) $mode
            if (Get-Process -Name 'cs2' -ErrorAction SilentlyContinue) {
                & $setStatus 'CS2 is running and may overwrite this change.' 'Warning'
                $answer = [Windows.Forms.MessageBox]::Show('CS2 is running and may overwrite this change when it closes. Apply anyway?', 'CS2 is running', 'YesNo', 'Warning')
                if ($answer -ne 'Yes') { return }
            }
            $updateResult = Update-VideoConfig -Path $path -Resolution $selected -CreateBackup ([bool]$backupCheck.Checked)
            $form.Tag = [pscustomobject]@{ Path = $path; Resolution = $selected; Result = $updateResult }
            & $refreshCurrent
            if ($updateResult.Changed) {
                $applyStatus = "Changes applied: $($selected.Display)."
                $applyKind = 'Success'
                if ($updateResult.RetentionWarning) {
                    $applyStatus += " $($updateResult.RetentionWarning)"
                    $applyKind = 'Warning'
                }
                $applyDetails = if ($updateResult.BackupPath) { "Backup: $($updateResult.BackupPath)" } else { '' }
                & $setStatus $applyStatus $applyKind $applyDetails
            } else { & $setStatus 'The selected settings are already active.' 'Success' }
        } catch {
            $uiState.PathValid = $false; $uiState.CurrentConfig = $null
            $currentLabel.Text = 'Configuration file is not valid or cannot be read.'
            $currentLabel.BackColor = $theme.ErrorBack; $currentLabel.ForeColor = $theme.ErrorText
            & $updatePendingState
            & $setStatus $_.Exception.Message 'Error'
        }
    })

    $updateResponsiveLayout = {
        Set-Cs2ResponsiveEditorLayout $form $body $accountCard $displayCard $statusLabel $uiState
    }
    $form.Add_Resize($updateResponsiveLayout)

    if ($accountBox.Items.Count -gt 0) {
        $selectedAccount = $SteamAccounts | Where-Object ConfigPath -eq $InitialPath | Select-Object -First 1
        if ($selectedAccount) { $accountBox.SelectedItem = $selectedAccount } else { $accountBox.SelectedIndex = 0 }
        & $updateAccountAccessibility
    }
    $ratioButtons['1'].Checked = $true
    & $updateRatioStyles
    $uiState.IsWide = $false
    & $updateResponsiveLayout
    & $refreshCurrent
    if ($Preferences.Warning) { & $setStatus $Preferences.Warning 'Warning' }
    [void]$form.ShowDialog()
    return $form.Tag
    } finally {
        if ($statusToolTip) { $statusToolTip.Dispose() }
        if ($accountToolTip) { $accountToolTip.Dispose() }
        if ($form) { $form.Dispose() }
        $fontNormal.Dispose()
        $fontSmall.Dispose()
        $fontSection.Dispose()
        $fontPreview.Dispose()
        $fontTitle.Dispose()
    }
}

function Invoke-Cs2VideoConfigEditor {
    param([hashtable]$BoundParameters)
    try {
        if ($Silent -and -not $Preset -and -not $ListAccounts -and -not $ExportDiagnostics) {
            throw '-Silent requires -Preset, -ListAccounts, or -ExportDiagnostics because interactive interfaces cannot be silent.'
        }
        if ($ExportDiagnostics -and ($Preset -or $Console -or $ListAccounts)) {
            throw '-ExportDiagnostics cannot be combined with -Preset, -Console, or -ListAccounts.'
        }
        $steamRoots = @(Get-SteamRoots $SteamRoot)
        $steamAccounts = Get-SteamAccounts -Roots $steamRoots
        $discoveredPaths = @($steamAccounts | Where-Object HasConfig | Select-Object -ExpandProperty ConfigPath)
        if ($ListAccounts) {
            $steamAccounts | Select-Object PersonaName, AccountId, SteamId64, HasConfig, ConfigPath
            exit 0
        }
        if ($BoundParameters.ContainsKey('FilePath')) { $script:SelectedConfigPath = [IO.Path]::GetFullPath($FilePath) }
        elseif ($discoveredPaths.Count -gt 0) { $script:SelectedConfigPath = $discoveredPaths[0] }

        if ($ExportDiagnostics) {
            $diagnosticPath = Export-Cs2Diagnostics `
                -Path $ExportDiagnostics `
                -ApplicationVersion $script:ApplicationVersion `
                -SteamRoots $steamRoots `
                -Accounts $steamAccounts `
                -SelectedPath $script:SelectedConfigPath
            if (-not $Silent) { Write-Output "Diagnostic report: $diagnosticPath" }
            exit 0
        } elseif ($Preset) {
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
            $settingsPath = Get-Cs2SettingsPath
            $preferences = Read-Cs2Preferences $settingsPath
            if (-not $BoundParameters.ContainsKey('FilePath')) {
                $preferred = $steamAccounts | Where-Object AccountId -eq $preferences.LastAccountId | Select-Object -First 1
                if ($preferred) { $script:SelectedConfigPath = $preferred.ConfigPath }
            }
            foreach ($recentPath in @($preferences.RecentConfigPaths)) {
                if ($recentPath -notin $discoveredPaths -and $recentPath -ne $script:SelectedConfigPath) {
                    $steamAccounts = @($steamAccounts) + @(New-CustomSteamAccount $recentPath)
                }
            }
            if ($script:SelectedConfigPath -and $script:SelectedConfigPath -notin $discoveredPaths -and
                -not ($steamAccounts | Where-Object ConfigPath -eq $script:SelectedConfigPath)) {
                $steamAccounts = @(New-CustomSteamAccount $script:SelectedConfigPath) + $steamAccounts
            }
            $dialogOutput = @(Show-ModernGraphicalEditor $steamAccounts $script:SelectedConfigPath $preferences $steamRoots $settingsPath)
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
        if (-not $Preset -and -not $Console -and -not $ListAccounts -and -not $ExportDiagnostics) {
            try {
                Add-Type -AssemblyName System.Windows.Forms
                [void][Windows.Forms.MessageBox]::Show($_.Exception.Message, 'CS2 Video Configuration', 'OK', 'Error')
            } catch { }
        }
        Write-Error $_.Exception.Message -ErrorAction Continue
        exit 1
    }
}

if ($MyInvocation.InvocationName -ne '.') {
    Invoke-Cs2VideoConfigEditor ([hashtable]$PSBoundParameters)
}
