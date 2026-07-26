$ErrorActionPreference = 'Stop'
$script:SteamId64Base = [uint64]76561197960265728
$script:Cs2ConfigRelativePath = '730\local\cfg\cs2_video.txt'

function ConvertTo-SteamId64 {
    param([uint32]$AccountId)
    ([uint64]$AccountId + $script:SteamId64Base).ToString()
}

function Format-SteamAccountDetails {
    param([psobject]$Account)
    if ($null -eq $Account -or [string]::IsNullOrWhiteSpace([string]$Account.AccountId)) {
        return [string]$Account.PersonaName
    }
    "$($Account.PersonaName) | $(Format-SteamAccountIdentifiers $Account)"
}

function Format-SteamAccountIdentifiers {
    param([psobject]$Account)
    if ($null -eq $Account -or [string]::IsNullOrWhiteSpace([string]$Account.AccountId)) {
        if ($Account -and $Account.ConfigPath) { return [string]$Account.ConfigPath }
        return 'Configuration selected manually'
    }
    "Account ID $($Account.AccountId) | SteamID64 $($Account.SteamId64)"
}

function New-CustomSteamAccount {
    param([string]$Path)
    [pscustomobject]@{
        DisplayName = 'Custom configuration file'
        AccountId = $null
        SteamId64 = $null
        PersonaName = 'Custom file'
        AccountName = $null
        MostRecent = $false
        ConfigPath = $Path
        HasConfig = (Test-Path -LiteralPath $Path -PathType Leaf)
        LastWriteTime = [datetime]::MinValue
    }
}

function Merge-Cs2AccountChoices {
    param([object[]]$Accounts, [string[]]$RecentConfigPaths)
    $combined = New-Object System.Collections.Generic.List[object]
    $knownPaths = @{}
    foreach ($account in @($Accounts)) {
        [void]$combined.Add($account)
        if (-not [string]::IsNullOrWhiteSpace([string]$account.ConfigPath)) {
            $knownPaths[[IO.Path]::GetFullPath($account.ConfigPath).ToUpperInvariant()] = $true
        }
    }
    foreach ($recentPath in @($RecentConfigPaths)) {
        if ([string]::IsNullOrWhiteSpace([string]$recentPath)) { continue }
        $fullPath = [IO.Path]::GetFullPath($recentPath)
        $key = $fullPath.ToUpperInvariant()
        if (-not $knownPaths.ContainsKey($key)) {
            [void]$combined.Add((New-CustomSteamAccount $fullPath))
            $knownPaths[$key] = $true
        }
    }
    @($combined | ForEach-Object { $_ })
}

function Get-SteamRoots {
    param([string]$Override)
    if ($Override) {
        if (-not (Test-Path -LiteralPath $Override -PathType Container)) { throw "Steam folder not found: $Override" }
        return [IO.Path]::GetFullPath($Override)
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
    $match = [regex]::Match($Text, '(?m)^[\t ]*"' + [regex]::Escape($Name) + '"[\t ]*"(?<value>(?:\\.|[^"\\])*)"[\t ]*\r?$')
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
    param([string[]]$Roots = @(Get-SteamRoots))
    $accounts = foreach ($root in $Roots) {
        $loginUsers = @{}
        foreach ($user in Get-SteamLoginUsers $root) { $loginUsers[[string]$user.SteamId64] = $user }
        $userdata = Join-Path $root 'userdata'
        if (-not (Test-Path -LiteralPath $userdata -PathType Container)) { continue }

        foreach ($directory in Get-ChildItem -LiteralPath $userdata -Directory -ErrorAction SilentlyContinue) {
            $accountId = [uint32]0
            if (-not [uint32]::TryParse($directory.Name, [ref]$accountId)) { continue }
            $steamId64 = ConvertTo-SteamId64 $accountId
            $login = $loginUsers[[string]$steamId64]
            $configPath = Join-Path $directory.FullName $script:Cs2ConfigRelativePath
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

Export-ModuleMember -Function @(
    'ConvertTo-SteamId64',
    'Format-SteamAccountDetails',
    'Format-SteamAccountIdentifiers',
    'New-CustomSteamAccount',
    'Merge-Cs2AccountChoices',
    'Get-SteamRoots',
    'Get-VdfField',
    'Get-SteamLoginUsers',
    'Get-SteamAccounts'
)
