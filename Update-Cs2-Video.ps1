# Update-CS2 video config: preserve exact formatting, change only values
# Hard-coded path
$ErrorActionPreference = 'Stop'
$ConfigPath = "C:\Program Files (x86)\Steam\userdata\2421650\730\local\cfg\cs2_video.txt"

function Pause-Exit([int]$code = 0) {
  try { Read-Host "Press Enter to exit" | Out-Null } catch {}
  exit $code
}

try {
  if (-not (Test-Path -LiteralPath $ConfigPath)) {
    throw "File not found: $ConfigPath"
  }

  # Read all lines without altering line endings
  $lines = Get-Content -LiteralPath $ConfigPath -Encoding Byte
  # Detect EOL
  $text = [System.Text.Encoding]::UTF8.GetString($lines)
  if ($text -notmatch "\r\n" -and $text -match "\n") {
    $eol = "`n"
  } else {
    $eol = "`r`n"
  }
  $content = $text -split "\r?\n", 0, 'Singleline'

  Write-Host "Choose aspect ratio:"
  Write-Host "  1) 4:3  -> setting.aspectratiomode = 0"
  Write-Host "  2) 16:9 -> setting.aspectratiomode = 1"
  Write-Host "  3) 16:10 -> setting.aspectratiomode = 2"
  $choice = Read-Host "Enter 1, 2, or 3"
  $aspectMap = @{ '1' = '0'; '2' = '1'; '3' = '2' }
  if (-not $aspectMap.ContainsKey($choice)) { throw "Invalid selection." }
  $aspectValue = $aspectMap[$choice]

  do { $h = Read-Host "Enter horizontal resolution (e.g., 1920)" }
  while (-not ($h -as [int]) -or [int]$h -le 0)
  do { $v = Read-Host "Enter vertical resolution (e.g., 1080)" }
  while (-not ($v -as [int]) -or [int]$v -le 0)

  # Replace only the last quoted value for a given key on the line
  function Set-ValuePreserveFormatting {
    param(
      [string[]]$AllLines,
      [string]$Key,      # e.g., setting.defaultres
      [string]$NewValue
    )
    # Pattern:
    #  - start optional space, opening quote
    #  - capture key not including quote
    #  - then anything until the final quoted value
    #  - replace the final quoted value only
    $pattern = '(^\s*"' + [regex]::Escape($Key) + '"(?:.*))"[^"]*"\s*$'
    for ($i = 0; $i -lt $AllLines.Count; $i++) {
      $line = $AllLines[$i]
      if ($line -match $pattern) {
        $prefix = $Matches[1]
        $AllLines[$i] = $prefix + '"' + $NewValue + '"'
        return ,$AllLines
      }
    }
    # If not found, do not inject (to keep file identical except values)
    return ,$AllLines
  }

  # Backup original bytes
  $ts = Get-Date -Format "yyyyMMdd_HHmmss"
  $backupPath = "$ConfigPath.$ts.bak"
  [IO.File]::WriteAllBytes($backupPath, $lines)
  Write-Host "Backup created: $backupPath" -ForegroundColor Yellow

  # Apply changes while preserving formatting quirks like: "" "value"
  $content = Set-ValuePreserveFormatting -AllLines $content -Key "setting.aspectratiomode" -NewValue $aspectValue
  $content = Set-ValuePreserveFormatting -AllLines $content -Key "setting.defaultres" -NewValue ([string][int]$h)
  $content = Set-ValuePreserveFormatting -AllLines $content -Key "setting.defaultresheight" -NewValue ([string][int]$v)

  # Join with original line endings and write back as UTF-8 without BOM changes
  $newText = [string]::Join($eol, $content)
  $newBytes = [System.Text.Encoding]::UTF8.GetBytes($newText)
  [IO.File]::WriteAllBytes($ConfigPath, $newBytes)

  Write-Host "Updated $ConfigPath"
  Write-Host "Set aspectratiomode=$aspectValue, resolution=${h}x${v}"
  Exit 0

} catch {
  Write-Host "Error: $($_.Exception.Message)" -ForegroundColor Red
  if ($_.InvocationInfo.PositionMessage) {
    Write-Host "`nAt: $($_.InvocationInfo.PositionMessage)" -ForegroundColor DarkGray
  }
  Pause-Exit 0
}