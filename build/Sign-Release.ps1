[CmdletBinding(SupportsShouldProcess)]
param(
    [Parameter(Mandatory)][string]$CertificateThumbprint,
    [Parameter(Mandatory)][string]$ExpectedCertificateSubject,
    [string]$Executable = (Join-Path $PSScriptRoot '..\dist\CS2-ResEdit.exe'),
    [string]$TimestampServer = 'http://timestamp.digicert.com'
)

$ErrorActionPreference = 'Stop'
$certificate = Get-Item -LiteralPath "Cert:\CurrentUser\My\$CertificateThumbprint"
if (-not (Test-Path -LiteralPath $Executable -PathType Leaf)) { throw 'Executable was not found.' }
if ($certificate.Subject -cne $ExpectedCertificateSubject) {
    throw 'The selected certificate subject does not match the expected publisher.'
}
if (-not $certificate.HasPrivateKey) { throw 'The selected certificate has no private key.' }
if ($certificate.NotBefore -gt (Get-Date) -or $certificate.NotAfter -le (Get-Date)) {
    throw 'The selected certificate is not currently valid.'
}
$codeSigningOid = '1.3.6.1.5.5.7.3.3'
if (@($certificate.EnhancedKeyUsageList | ForEach-Object ObjectId) -notcontains $codeSigningOid) {
    throw 'The selected certificate is not authorized for code signing.'
}
$chain = [Security.Cryptography.X509Certificates.X509Chain]::new()
if (-not $chain.Build($certificate)) {
    throw 'The selected certificate does not have a trusted certificate chain.'
}
if ($PSCmdlet.ShouldProcess($Executable, 'Apply Authenticode signature')) {
    $signature = Set-AuthenticodeSignature -LiteralPath $Executable -Certificate $certificate `
        -HashAlgorithm SHA256 -TimestampServer $TimestampServer
    if ($signature.Status -ne 'Valid') { throw "Signing failed: $($signature.StatusMessage)" }
    $verified = Get-AuthenticodeSignature -LiteralPath $Executable
    if ($verified.Status -ne 'Valid' -or $verified.SignerCertificate.Thumbprint -ne $certificate.Thumbprint) {
        throw 'The executable signature could not be verified against the selected certificate.'
    }
    $hash = (Get-FileHash -LiteralPath $Executable -Algorithm SHA256).Hash.ToLowerInvariant()
    [IO.File]::WriteAllText(
        "$Executable.sha256",
        "$hash  $([IO.Path]::GetFileName($Executable))`n",
        [Text.UTF8Encoding]::new($false)
    )
    $verified
}
