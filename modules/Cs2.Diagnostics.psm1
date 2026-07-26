$ErrorActionPreference = 'Stop'
$script:DiagnosticSchemaVersion = 1
$script:DiagnosticEventLimit = 20

function New-Cs2DiagnosticContext {
    param(
        [string[]]$SteamRoots = @(),
        [object[]]$Accounts = @()
    )

    $knownRoots = New-Object System.Collections.Generic.List[object]
    $rootCandidates = @(
        [pscustomobject]@{ Path = [Environment]::GetFolderPath('UserProfile'); Alias = '<USER_PROFILE>' }
        [pscustomobject]@{ Path = [Environment]::GetFolderPath('LocalApplicationData'); Alias = '<LOCAL_APP_DATA>' }
    )
    $steamIndex = 0
    foreach ($steamRoot in @($SteamRoots)) {
        if ([string]::IsNullOrWhiteSpace([string]$steamRoot)) { continue }
        $steamIndex++
        $rootCandidates += [pscustomobject]@{
            Path = [string]$steamRoot
            Alias = "<STEAM_ROOT_$steamIndex>"
        }
    }
    foreach ($candidate in $rootCandidates) {
        if ([string]::IsNullOrWhiteSpace([string]$candidate.Path)) { continue }
        try { $fullPath = [IO.Path]::GetFullPath([string]$candidate.Path).TrimEnd('\', '/') }
        catch { continue }
        if ([string]::IsNullOrWhiteSpace($fullPath)) { continue }
        if (-not ($knownRoots | Where-Object Path -eq $fullPath)) {
            [void]$knownRoots.Add([pscustomobject]@{ Path = $fullPath; Alias = $candidate.Alias })
        }
    }

    $sensitiveValues = New-Object System.Collections.Generic.List[string]
    foreach ($account in @($Accounts)) {
        foreach ($propertyName in @('PersonaName', 'AccountName', 'AccountId', 'SteamId64')) {
            if ($null -eq $account -or $null -eq $account.PSObject.Properties[$propertyName]) { continue }
            $value = [string]$account.$propertyName
            if (-not [string]::IsNullOrWhiteSpace($value) -and $value.Length -ge 3 -and
                -not $sensitiveValues.Contains($value)) {
                [void]$sensitiveValues.Add($value)
            }
        }
    }

    [pscustomobject]@{
        KnownRoots = @($knownRoots | Sort-Object { $_.Path.Length } -Descending)
        SensitiveValues = @($sensitiveValues | Sort-Object Length -Descending)
        CustomPaths = @{}
        NextCustomPath = 1
        Operations = New-Object System.Collections.ArrayList
    }
}

function ConvertTo-Cs2DiagnosticPath {
    param(
        [Parameter(Mandatory)]
        [psobject]$Context,

        [AllowNull()]
        [string]$Path
    )

    if ([string]::IsNullOrWhiteSpace($Path)) { return $null }
    try { $fullPath = [IO.Path]::GetFullPath($Path) }
    catch { return Protect-Cs2DiagnosticText -Context $Context -Text $Path }

    foreach ($knownRoot in @($Context.KnownRoots)) {
        if ($fullPath.Equals($knownRoot.Path, [StringComparison]::OrdinalIgnoreCase)) {
            return [string]$knownRoot.Alias
        }
        $prefix = $knownRoot.Path.TrimEnd('\', '/') + [IO.Path]::DirectorySeparatorChar
        if ($fullPath.StartsWith($prefix, [StringComparison]::OrdinalIgnoreCase)) {
            $relative = $fullPath.Substring($prefix.Length)
            $safePath = ([string]$knownRoot.Alias + '\' + $relative)
            return [regex]::Replace($safePath, '(?i)(\\userdata\\)\d+(?=\\|$)', '${1}<ACCOUNT_ID>')
        }
    }

    $key = $fullPath.ToUpperInvariant()
    if (-not $Context.CustomPaths.ContainsKey($key)) {
        $Context.CustomPaths[$key] = "<CUSTOM_PATH_$($Context.NextCustomPath)>"
        $Context.NextCustomPath = [int]$Context.NextCustomPath + 1
    }
    $alias = [string]$Context.CustomPaths[$key]
    $fileName = [IO.Path]::GetFileName($fullPath)
    if ($fileName -and $fileName -ieq 'cs2_video.txt') { return "$alias\cs2_video.txt" }
    $alias
}

function Protect-Cs2DiagnosticText {
    param(
        [Parameter(Mandatory)]
        [psobject]$Context,

        [AllowNull()]
        [string]$Text
    )

    if ($null -eq $Text) { return $null }
    $safe = [string]$Text
    foreach ($knownRoot in @($Context.KnownRoots)) {
        $safe = [regex]::Replace(
            $safe,
            [regex]::Escape([string]$knownRoot.Path),
            [string]$knownRoot.Alias,
            [Text.RegularExpressions.RegexOptions]::IgnoreCase
        )
    }
    foreach ($sensitiveValue in @($Context.SensitiveValues)) {
        $safe = [regex]::Replace(
            $safe,
            [regex]::Escape([string]$sensitiveValue),
            '<REDACTED>',
            [Text.RegularExpressions.RegexOptions]::IgnoreCase
        )
    }
    $safe = [regex]::Replace($safe, '(?i)(\\userdata\\)\d+(?=\\|$)', '${1}<ACCOUNT_ID>')
    $safe = [regex]::Replace($safe, '(?<!\d)7656119\d{10}(?!\d)', '<STEAM_ID_64>')

    $pathPattern = '(?i)(?:[A-Z]:\\|\\\\)[^"''\r\n|;]+'
    $safe = [regex]::Replace($safe, $pathPattern, {
        param($match)
        ConvertTo-Cs2DiagnosticPath -Context $Context -Path $match.Value.Trim()
    })
    $safe
}

function Add-Cs2DiagnosticEvent {
    param(
        [Parameter(Mandatory)]
        [psobject]$Context,

        [Parameter(Mandatory)]
        [ValidateNotNullOrEmpty()]
        [string]$Kind,

        [Parameter(Mandatory)]
        [ValidateSet('Success', 'Warning', 'Error', 'Info')]
        [string]$Outcome,

        [ValidateNotNullOrEmpty()]
        [string]$Code = 'unspecified',

        [AllowNull()]
        [string]$Details
    )

    $eventRecord = [pscustomobject][ordered]@{
        TimestampUtc = [datetime]::UtcNow.ToString('o', [Globalization.CultureInfo]::InvariantCulture)
        Kind = Protect-Cs2DiagnosticText -Context $Context -Text $Kind
        Outcome = $Outcome
        Code = Protect-Cs2DiagnosticText -Context $Context -Text $Code
        Details = Protect-Cs2DiagnosticText -Context $Context -Text $Details
    }
    [void]$Context.Operations.Add($eventRecord)
    while ($Context.Operations.Count -gt $script:DiagnosticEventLimit) {
        $Context.Operations.RemoveAt(0)
    }
    $eventRecord
}

function Export-Cs2Diagnostics {
    param(
        [Parameter(Mandatory)]
        [ValidateNotNullOrEmpty()]
        [string]$Path,

        [Parameter(Mandatory)]
        [ValidateNotNullOrEmpty()]
        [string]$ApplicationVersion,

        [string[]]$SteamRoots = @(),

        [object[]]$Accounts = @(),

        [AllowNull()]
        [string]$SelectedPath,

        [object[]]$Operations = @()
    )

    $context = New-Cs2DiagnosticContext -SteamRoots $SteamRoots -Accounts $Accounts
    [void](Add-Cs2DiagnosticEvent -Context $context -Kind 'Discovery' -Outcome 'Success' -Code 'scan-complete' -Details "$(@($Accounts).Count) account entries found.")
    foreach ($operation in @($Operations | Select-Object -Last $script:DiagnosticEventLimit)) {
        $kind = if ($operation.Kind) { [string]$operation.Kind } else { 'Operation' }
        $outcome = if ([string]$operation.Outcome -in @('Success', 'Warning', 'Error', 'Info')) {
            [string]$operation.Outcome
        } else {
            'Info'
        }
        $code = if ($operation.Code) { [string]$operation.Code } else { 'unspecified' }
        [void](Add-Cs2DiagnosticEvent -Context $context -Kind $kind -Outcome $outcome -Code $code -Details ([string]$operation.Details))
    }

    $selected = [ordered]@{
        Path = ConvertTo-Cs2DiagnosticPath -Context $context -Path $SelectedPath
        Exists = $false
        Readable = $false
        Encoding = $null
        Width = $null
        Height = $null
        Mode = $null
        ValidationError = $null
    }
    if (-not [string]::IsNullOrWhiteSpace($SelectedPath)) {
        $selected.Exists = Test-Path -LiteralPath $SelectedPath -PathType Leaf
        if ($selected.Exists) {
            try {
                $file = Get-TextFileInfo -Path $SelectedPath
                $config = Get-CurrentConfig -Path $SelectedPath
                $selected.Readable = $true
                $selected.Encoding = [string]$file.Encoding.WebName
                $selected.Width = [int]$config.Width
                $selected.Height = [int]$config.Height
                $selected.Mode = [string]$config.Mode
                [void](Add-Cs2DiagnosticEvent -Context $context -Kind 'Configuration' -Outcome 'Success' -Code 'validation-passed')
            } catch {
                $selected.ValidationError = Protect-Cs2DiagnosticText -Context $context -Text $_.Exception.Message
                [void](Add-Cs2DiagnosticEvent -Context $context -Kind 'Configuration' -Outcome 'Error' -Code 'validation-failed' -Details $_.Exception.Message)
            }
        } else {
            $selected.ValidationError = 'The selected configuration file does not exist.'
            [void](Add-Cs2DiagnosticEvent -Context $context -Kind 'Configuration' -Outcome 'Warning' -Code 'file-not-found')
        }
    }

    $safeAccounts = New-Object System.Collections.Generic.List[object]
    $accountIndex = 0
    foreach ($account in @($Accounts)) {
        $accountIndex++
        [void]$safeAccounts.Add([pscustomobject][ordered]@{
            Alias = "Account $accountIndex"
            HasConfig = [bool]$account.HasConfig
            MostRecent = [bool]$account.MostRecent
            ConfigPath = ConvertTo-Cs2DiagnosticPath -Context $context -Path ([string]$account.ConfigPath)
        })
    }

    $report = [ordered]@{
        SchemaVersion = $script:DiagnosticSchemaVersion
        GeneratedUtc = [datetime]::UtcNow.ToString('o', [Globalization.CultureInfo]::InvariantCulture)
        App = [ordered]@{ Version = $ApplicationVersion }
        Runtime = [ordered]@{
            PSVersion = $PSVersionTable.PSVersion.ToString()
            PSEdition = [string]$PSVersionTable.PSEdition
            OSVersion = [Environment]::OSVersion.VersionString
            Is64BitProcess = [Environment]::Is64BitProcess
        }
        Discovery = [ordered]@{
            RootCount = @($SteamRoots).Count
            AccountCount = @($Accounts).Count
            Accounts = $safeAccounts.ToArray()
        }
        SelectedConfig = $selected
        Operations = [object[]]($context.Operations | Select-Object -Last $script:DiagnosticEventLimit)
        Privacy = [ordered]@{
            PathsAnonymized = $true
            PersonalIdentifiersIncluded = $false
        }
    }

    $fullPath = [IO.Path]::GetFullPath($Path)
    $directory = [IO.Path]::GetDirectoryName($fullPath)
    [void][IO.Directory]::CreateDirectory($directory)
    $tempPath = Join-Path $directory ('.diagnostics-' + [guid]::NewGuid().ToString('N') + '.tmp')
    try {
        $json = $report | ConvertTo-Json -Depth 8
        [IO.File]::WriteAllText($tempPath, $json, (New-Object Text.UTF8Encoding($false)))
        if (Test-Path -LiteralPath $fullPath -PathType Leaf) {
            $replaceBackup = "$tempPath.replace"
            try {
                [IO.File]::Replace($tempPath, $fullPath, $replaceBackup)
            } finally {
                if (Test-Path -LiteralPath $replaceBackup) {
                    Remove-Item -LiteralPath $replaceBackup -Force -ErrorAction SilentlyContinue
                }
            }
        } else {
            [IO.File]::Move($tempPath, $fullPath)
        }
    } finally {
        if (Test-Path -LiteralPath $tempPath) {
            Remove-Item -LiteralPath $tempPath -Force -ErrorAction SilentlyContinue
        }
    }
    $fullPath
}

Export-ModuleMember -Function @(
    'New-Cs2DiagnosticContext',
    'ConvertTo-Cs2DiagnosticPath',
    'Protect-Cs2DiagnosticText',
    'Add-Cs2DiagnosticEvent',
    'Export-Cs2Diagnostics'
)
