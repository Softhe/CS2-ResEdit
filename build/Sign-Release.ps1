[CmdletBinding(SupportsShouldProcess)]
param(
    [Parameter(Mandatory)][string]$CertificateThumbprint,
    [string]$Executable = (Join-Path $PSScriptRoot '..\dist\CS2-ResEdit.exe'),
    [string]$TimestampServer = 'http://timestamp.digicert.com'
)

$ErrorActionPreference = 'Stop'
$certificate = Get-Item -LiteralPath "Cert:\CurrentUser\My\$CertificateThumbprint"
if ($PSCmdlet.ShouldProcess($Executable, 'Apply Authenticode signature')) {
    $signature = Set-AuthenticodeSignature -LiteralPath $Executable -Certificate $certificate `
        -HashAlgorithm SHA256 -TimestampServer $TimestampServer
    if ($signature.Status -ne 'Valid') { throw "Signing failed: $($signature.StatusMessage)" }
    $hash = (Get-FileHash -LiteralPath $Executable -Algorithm SHA256).Hash.ToLowerInvariant()
    [IO.File]::WriteAllText(
        "$Executable.sha256",
        "$hash  $([IO.Path]::GetFileName($Executable))`n",
        [Text.UTF8Encoding]::new($false)
    )
    $signature
}
