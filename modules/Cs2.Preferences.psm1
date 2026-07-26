$ErrorActionPreference = 'Stop'
$script:PreferenceSchemaVersion = 1

function Get-Cs2SettingsPath {
    $basePath = [Environment]::GetFolderPath('LocalApplicationData')
    if ([string]::IsNullOrWhiteSpace($basePath)) { throw 'LocalAppData is unavailable.' }
    Join-Path $basePath 'Softhe\CS2-VideoConfig-Editor\settings.json'
}

function New-Cs2Preferences {
    [pscustomobject]@{
        SchemaVersion = $script:PreferenceSchemaVersion
        LastAccountId = $null
        RecentConfigPaths = @()
        Warning = $null
    }
}

function Read-Cs2Preferences {
    param([string]$Path = (Get-Cs2SettingsPath))
    $preferences = New-Cs2Preferences
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) { return $preferences }
    try {
        $data = Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json
        if ([int]$data.SchemaVersion -ne $script:PreferenceSchemaVersion) {
            throw "Unsupported preference schema '$($data.SchemaVersion)'."
        }
        if ($data.LastAccountId) { $preferences.LastAccountId = [string]$data.LastAccountId }
        $seen = @{}
        $recentPaths = New-Object System.Collections.Generic.List[string]
        foreach ($candidate in @($data.RecentConfigPaths)) {
            if ([string]::IsNullOrWhiteSpace([string]$candidate)) { continue }
            $fullPath = [IO.Path]::GetFullPath([string]$candidate)
            $key = $fullPath.ToUpperInvariant()
            if ($seen.ContainsKey($key) -or -not (Test-Path -LiteralPath $fullPath -PathType Leaf)) { continue }
            try { [void](Get-CurrentConfig $fullPath) } catch { continue }
            $seen[$key] = $true
            [void]$recentPaths.Add($fullPath)
            if ($recentPaths.Count -ge 5) { break }
        }
        $preferences.RecentConfigPaths = @($recentPaths)
    } catch {
        $preferences.Warning = "Preferences could not be loaded: $($_.Exception.Message)"
    }
    $preferences
}

function Save-Cs2Preferences {
    param(
        [string]$LastAccountId,
        [string[]]$RecentConfigPaths,
        [string]$Path = (Get-Cs2SettingsPath)
    )
    $directory = [IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($Path))
    [void][IO.Directory]::CreateDirectory($directory)
    $seen = @{}
    $cleanPaths = New-Object System.Collections.Generic.List[string]
    foreach ($candidate in @($RecentConfigPaths)) {
        if ([string]::IsNullOrWhiteSpace([string]$candidate)) { continue }
        $fullPath = [IO.Path]::GetFullPath([string]$candidate)
        $key = $fullPath.ToUpperInvariant()
        if ($seen.ContainsKey($key) -or -not (Test-Path -LiteralPath $fullPath -PathType Leaf)) { continue }
        try { [void](Get-CurrentConfig $fullPath) } catch { continue }
        $seen[$key] = $true
        [void]$cleanPaths.Add($fullPath)
        if ($cleanPaths.Count -ge 5) { break }
    }
    $payload = [ordered]@{
        SchemaVersion = $script:PreferenceSchemaVersion
        LastAccountId = if ([string]::IsNullOrWhiteSpace($LastAccountId)) { $null } else { $LastAccountId }
        RecentConfigPaths = @($cleanPaths)
    }
    $json = $payload | ConvertTo-Json -Depth 3
    $tempPath = Join-Path $directory ('.settings-' + [guid]::NewGuid().ToString('N') + '.tmp')
    try {
        [IO.File]::WriteAllText($tempPath, $json, (New-Object Text.UTF8Encoding($false)))
        if (Test-Path -LiteralPath $Path -PathType Leaf) {
            $replaceBackup = "$tempPath.replace"
            try { [IO.File]::Replace($tempPath, [IO.Path]::GetFullPath($Path), $replaceBackup) }
            finally { if (Test-Path -LiteralPath $replaceBackup) { Remove-Item -LiteralPath $replaceBackup -Force -ErrorAction SilentlyContinue } }
        } else {
            [IO.File]::Move($tempPath, [IO.Path]::GetFullPath($Path))
        }
    } finally {
        if (Test-Path -LiteralPath $tempPath) { Remove-Item -LiteralPath $tempPath -Force -ErrorAction SilentlyContinue }
    }
    [pscustomobject]@{
        SchemaVersion = $script:PreferenceSchemaVersion
        LastAccountId = $payload.LastAccountId
        RecentConfigPaths = @($cleanPaths)
    }
}

function Add-Cs2RecentConfigPath {
    param([string[]]$Paths, [string]$Path)
    $ordered = @($Path) + @($Paths)
    $seen = @{}
    $result = New-Object System.Collections.Generic.List[string]
    foreach ($candidate in $ordered) {
        if ([string]::IsNullOrWhiteSpace([string]$candidate)) { continue }
        $fullPath = [IO.Path]::GetFullPath([string]$candidate)
        $key = $fullPath.ToUpperInvariant()
        if ($seen.ContainsKey($key)) { continue }
        $seen[$key] = $true
        [void]$result.Add($fullPath)
        if ($result.Count -ge 5) { break }
    }
    @($result)
}

Export-ModuleMember -Function @(
    'Get-Cs2SettingsPath',
    'New-Cs2Preferences',
    'Read-Cs2Preferences',
    'Save-Cs2Preferences',
    'Add-Cs2RecentConfigPath'
)
