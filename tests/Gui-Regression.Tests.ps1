BeforeAll {
    $script:EditorPath = Join-Path $PSScriptRoot '..\CS2-VideoConfig-Editor.ps1'
    $script:EditorSource = [IO.File]::ReadAllText($script:EditorPath)
    . $script:EditorPath
    Initialize-Cs2GraphicalInterface

    function Get-ContrastRatio {
        param(
            [Drawing.Color]$Foreground,
            [Drawing.Color]$Background
        )

        function Get-RelativeLuminance {
            param([Drawing.Color]$Color)
            $channels = @($Color.R, $Color.G, $Color.B) | ForEach-Object {
                $channel = $_ / 255.0
                if ($channel -le 0.04045) {
                    $channel / 12.92
                } else {
                    [math]::Pow((($channel + 0.055) / 1.055), 2.4)
                }
            }
            (0.2126 * $channels[0]) + (0.7152 * $channels[1]) + (0.0722 * $channels[2])
        }

        $foregroundLuminance = Get-RelativeLuminance $Foreground
        $backgroundLuminance = Get-RelativeLuminance $Background
        $lighter = [math]::Max($foregroundLuminance, $backgroundLuminance)
        $darker = [math]::Min($foregroundLuminance, $backgroundLuminance)
        ($lighter + 0.05) / ($darker + 0.05)
    }

    function Assert-SourceContract {
        param(
            [Parameter(Mandatory)]
            [string]$Pattern,

            [Parameter(Mandatory)]
            [string]$Because
        )

        $script:EditorSource | Should -Match $Pattern -Because $Because
    }
}

Describe 'GUI responsive layout regression' -Tag 'GuiRegression' {
    It 'uses stable breakpoints from 100 through 200 percent scaling' {
        $cases = @(
            @{ Dpi = 96;  Breakpoint = 900 }
            @{ Dpi = 120; Breakpoint = 1125 }
            @{ Dpi = 144; Breakpoint = 1350 }
            @{ Dpi = 168; Breakpoint = 1575 }
            @{ Dpi = 192; Breakpoint = 1800 }
        )

        foreach ($case in $cases) {
            Get-Cs2ResponsiveBreakpoint $case.Dpi | Should -Be $case.Breakpoint
        }
    }

    It 'rejects implausible DPI values instead of producing a broken layout' {
        { Get-Cs2ResponsiveBreakpoint 47 } | Should -Throw '*Unsupported DPI*'
        { Get-Cs2ResponsiveBreakpoint 769 } | Should -Throw '*Unsupported DPI*'
    }

    It 'renders the wide grid at the breakpoint and the stacked grid below it' {
        $form = New-Object Windows.Forms.Form
        $body = New-Object Windows.Forms.TableLayoutPanel
        $accountCard = New-Object Windows.Forms.Panel
        $displayCard = New-Object Windows.Forms.Panel
        $status = New-Object Windows.Forms.Label
        $state = [pscustomobject]@{ IsWide = $false }

        try {
            $form.Controls.Add($body)
            $body.Controls.Add($accountCard)
            $body.Controls.Add($displayCard)
            $body.Controls.Add($status)
            $breakpoint = Get-Cs2ResponsiveBreakpoint $form.DeviceDpi

            $form.ClientSize = New-Object Drawing.Size($breakpoint, 700)
            Set-Cs2ResponsiveEditorLayout $form $body $accountCard $displayCard $status $state

            $state.IsWide | Should -BeTrue
            $body.ColumnCount | Should -Be 2
            $body.RowCount | Should -Be 2
            $body.ColumnStyles.Count | Should -Be 2
            $body.RowStyles.Count | Should -Be 2
            $body.ColumnStyles[0].Width | Should -Be 42
            $body.ColumnStyles[1].Width | Should -Be 58
            $body.GetCellPosition($accountCard).Column | Should -Be 0
            $body.GetCellPosition($displayCard).Column | Should -Be 1
            $body.GetCellPosition($status).Row | Should -Be 1
            $body.GetColumnSpan($status) | Should -Be 2
            $accountCard.Margin.Right | Should -Be 6
            $displayCard.Margin.Left | Should -Be 6

            $form.ClientSize = New-Object Drawing.Size(($breakpoint - 1), 700)
            Set-Cs2ResponsiveEditorLayout $form $body $accountCard $displayCard $status $state

            $state.IsWide | Should -BeFalse
            $body.ColumnCount | Should -Be 1
            $body.RowCount | Should -Be 3
            $body.ColumnStyles.Count | Should -Be 1
            $body.RowStyles.Count | Should -Be 3
            $body.RowStyles[0].Height | Should -Be 228
            $body.RowStyles[1].Height | Should -Be 420
            $body.RowStyles[2].Height | Should -Be 48
            $body.GetCellPosition($accountCard).Row | Should -Be 0
            $body.GetCellPosition($displayCard).Row | Should -Be 1
            $body.GetCellPosition($status).Row | Should -Be 2
            $body.GetColumnSpan($status) | Should -Be 1
            $accountCard.Margin.Right | Should -Be 0
            $displayCard.Margin.Left | Should -Be 0
        } finally {
            $form.Dispose()
        }
    }

    It 'does not accumulate grid styles when resize events repeat' {
        $form = New-Object Windows.Forms.Form
        $body = New-Object Windows.Forms.TableLayoutPanel
        $accountCard = New-Object Windows.Forms.Panel
        $displayCard = New-Object Windows.Forms.Panel
        $status = New-Object Windows.Forms.Label
        $state = [pscustomobject]@{ IsWide = $false }

        try {
            $form.Controls.Add($body)
            $body.Controls.Add($accountCard)
            $body.Controls.Add($displayCard)
            $body.Controls.Add($status)
            $form.ClientSize = New-Object Drawing.Size(1200, 700)

            1..5 | ForEach-Object {
                Set-Cs2ResponsiveEditorLayout $form $body $accountCard $displayCard $status $state
            }

            $body.ColumnStyles.Count | Should -Be 2
            $body.RowStyles.Count | Should -Be 2
            $body.GetColumnSpan($status) | Should -Be 2
        } finally {
            $form.Dispose()
        }
    }

    It 'fully repaints rounded cards after repeated height changes' {
        $theme = Get-Cs2UiTheme $false
        $parent = New-Object Windows.Forms.Panel
        $card = New-Cs2UiCard $theme
        try {
            $parent.Controls.Add($card)
            foreach ($height in @(480, 300, 520, 260, 500)) {
                $card.Size = New-Object Drawing.Size(640, $height)
                Set-Cs2UiRoundedRegion $card 12
            }

            $flags = [Reflection.BindingFlags]'Instance, NonPublic'
            $doubleBuffered = [Windows.Forms.Control].GetProperty('DoubleBuffered', $flags)
            $resizeRedraw = [Windows.Forms.Control].GetProperty('ResizeRedraw', $flags)
            $doubleBuffered.GetValue($card, $null) | Should -BeTrue
            $resizeRedraw.GetValue($card, $null) | Should -BeTrue
            $card.Region | Should -Not -BeNullOrEmpty
        } finally {
            $card.Dispose()
            $parent.Dispose()
        }
    }

    It 'scales display previews to preserve the selected aspect ratio' {
        $wide = Get-Cs2DisplayPreviewBounds 500 100 1920 1080 10
        $tall = Get-Cs2DisplayPreviewBounds 500 100 1280 1024 10

        $wide.Width / [double]$wide.Height | Should -BeGreaterThan 1.76
        $wide.Width / [double]$wide.Height | Should -BeLessThan 1.79
        $tall.Width / [double]$tall.Height | Should -BeGreaterThan 1.24
        $tall.Width / [double]$tall.Height | Should -BeLessThan 1.26
        $wide.Width | Should -BeGreaterThan $tall.Width
        (Get-Cs2DisplayPreviewBounds 10 10 1920 1080 10).IsEmpty | Should -BeTrue
    }
}

Describe 'GUI keyboard and accessibility contracts' -Tag 'GuiRegression' {
    It 'keeps the main account controls in a predictable tab sequence' {
        Assert-SourceContract '\$accountBox\.TabIndex\s*=\s*0' 'the account picker must be first'
        Assert-SourceContract '\$refreshAccounts\.TabIndex\s*=\s*1' 'refresh must follow the account picker'
        Assert-SourceContract '\$pathBox\.TabIndex\s*=\s*2' 'the path field must follow refresh'
        Assert-SourceContract '\$browse\.TabIndex\s*=\s*3' 'browse must follow the path field'
        Assert-SourceContract '\$backupsButton\.TabIndex\s*=\s*4' 'backup history must finish the account section'
    }

    It 'keeps display and footer controls in a predictable nested tab sequence' {
        Assert-SourceContract '\$aspectPanel\.TabIndex\s*=\s*0' 'aspect selection must lead the display section'
        Assert-SourceContract '\$radio\.TabIndex\s*=\s*\[int\]\$definition\.Mode' 'aspect buttons must follow visual order'
        Assert-SourceContract '\$resolutionBox\.TabIndex\s*=\s*1' 'resolution must follow aspect selection'
        Assert-SourceContract '\$customPanel\.TabIndex\s*=\s*2' 'custom dimensions must follow resolution'
        Assert-SourceContract '\$widthBox\.TabIndex\s*=\s*0' 'custom width must precede height'
        Assert-SourceContract '\$heightBox\.TabIndex\s*=\s*1' 'custom height must follow width'
        Assert-SourceContract '\$reset\.TabIndex\s*=\s*3' 'reset must finish the display section'
        Assert-SourceContract '\$backupCheck\.TabIndex\s*=\s*0' 'backup preference must lead the footer'
        Assert-SourceContract '\$buttonPanel\.TabIndex\s*=\s*1' 'actions must follow the backup preference'
        Assert-SourceContract '\$apply\.TabIndex\s*=\s*0' 'apply must lead the action group'
        Assert-SourceContract '\$cancel\.TabIndex\s*=\s*1' 'cancel must follow apply'
    }

    It 'maps Enter and Escape to the primary and cancel actions' {
        Assert-SourceContract '\$form\.AcceptButton\s*=\s*\$apply' 'Enter must invoke Apply'
        Assert-SourceContract '\$form\.CancelButton\s*=\s*\$cancel' 'Escape must invoke Cancel'
        Assert-SourceContract '\$cancel\.DialogResult\s*=\s*''Cancel''' 'Cancel must close the dialog safely'
    }

    It 'provides names for controls whose visible state alone is ambiguous' {
        $contracts = @(
            '\$form\.AccessibleDescription\s*='
            '\$accountBox\.AccessibleName\s*=\s*''Steam account'''
            '\$pathBox\.AccessibleName\s*=\s*''CS2 configuration file'''
            '\$resolutionBox\.AccessibleName\s*=\s*''Resolution'''
            '\$widthBox\.AccessibleName\s*=\s*''Custom width'''
            '\$heightBox\.AccessibleName\s*=\s*''Custom height'''
            '\$currentPreview\.AccessibleName\s*=\s*''Current display settings'''
            '\$pendingPreview\.AccessibleName\s*=\s*''Pending display settings'''
            '\$displayPreview\.AccessibleName\s*=\s*''Pending display shape preview'''
            '\$backupCheck\.AccessibleName\s*='
            '\$apply\.AccessibleName\s*='
            '\$statusLabel\.AccessibleName\s*='
            '\$statusLabel\.AccessibleDescription\s*='
        )

        foreach ($contract in $contracts) {
            Assert-SourceContract $contract 'screen readers need stable control and state descriptions'
        }
    }

    It 'retains readable status text and DPI-aware top-level sizing' {
        Assert-SourceContract '\$form\.AutoScaleMode\s*=\s*''Dpi''' 'the main form must scale with Windows DPI'
        Assert-SourceContract '\$dialog\.AutoScaleMode\s*=\s*''Dpi''' 'the backup dialog must scale with Windows DPI'
        Assert-SourceContract '\$form\.MinimumSize\s*=\s*New-Object Drawing\.Size\(700,\s*560\)' 'the editor needs a usable minimum viewport'
        Assert-SourceContract '\$body\.AutoScroll\s*=\s*\$true' 'narrow or enlarged content must remain reachable'
        Assert-SourceContract '\$statusLabel\.AutoEllipsis\s*=\s*\$true' 'long status text must not overlap adjacent UI'
        Assert-SourceContract '\$statusToolTip\.SetToolTip\(\$statusLabel' 'ellipsized status details must remain discoverable'
    }
}

Describe 'GUI color and state accessibility' -Tag 'GuiRegression' {
    It 'maps High Contrast roles exclusively to Windows system colors' {
        $theme = Get-Cs2UiTheme $true
        $expected = @{
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

        foreach ($role in $expected.Keys) {
            $theme.$role | Should -Be $expected[$role] -Because "$role must honor the active Windows High Contrast palette"
        }
    }

    It 'maintains useful contrast for the built-in dark theme' {
        $theme = Get-Cs2UiTheme $false
        Get-ContrastRatio $theme.Text $theme.Background | Should -BeGreaterOrEqual 7
        Get-ContrastRatio $theme.Muted $theme.Background | Should -BeGreaterOrEqual 4.5
        Get-ContrastRatio $theme.PrimaryText $theme.Primary | Should -BeGreaterOrEqual 3
        Get-ContrastRatio $theme.SuccessText $theme.SuccessBack | Should -BeGreaterOrEqual 4.5
        Get-ContrastRatio $theme.WarningText $theme.WarningBack | Should -BeGreaterOrEqual 4.5
        Get-ContrastRatio $theme.ErrorText $theme.ErrorBack | Should -BeGreaterOrEqual 4.5
    }

    It 'disables all mutation actions when the selected path is invalid' {
        $current = [pscustomobject]@{ Width = 1920; Height = 1080; Mode = '1' }
        $pending = [pscustomobject]@{ Width = 1280; Height = 960; Mode = '0' }
        $state = Get-Cs2PendingState $current $pending $false

        $state.PathValid | Should -BeFalse
        $state.Changed | Should -BeFalse
        $state.CanApply | Should -BeFalse
        $state.CanReset | Should -BeFalse
        $state.AlreadyActive | Should -BeFalse
    }

    It 'keeps Apply and Reset disabled until there is a valid change' {
        $current = [pscustomobject]@{ Width = 1920; Height = 1080; Mode = '1' }

        $unchanged = Get-Cs2PendingState $current $current $true
        $unchanged.CanApply | Should -BeFalse
        $unchanged.CanReset | Should -BeFalse
        $unchanged.AlreadyActive | Should -BeTrue

        $changed = Get-Cs2PendingState $current ([pscustomobject]@{ Width = 1600; Height = 900; Mode = '1' }) $true
        $changed.CanApply | Should -BeTrue
        $changed.CanReset | Should -BeTrue
        $changed.AlreadyActive | Should -BeFalse
    }
}
