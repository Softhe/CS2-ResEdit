[CmdletBinding(SupportsShouldProcess)]
param(
    [Parameter(Mandatory)]
    [string]$CertificateThumbprint,

    [string]$ManifestPath,

    [string]$TimestampServer = 'http://timestamp.digicert.com'
)

$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
if ([string]::IsNullOrWhiteSpace($ManifestPath)) {
    $ManifestPath = Join-Path $PSScriptRoot 'ReleaseFiles.psd1'
}
$ManifestPath = [IO.Path]::GetFullPath($ManifestPath)
if (-not (Test-Path -LiteralPath $ManifestPath -PathType Leaf)) {
    throw "Release file manifest was not found: $ManifestPath"
}

$normalizedThumbprint = $CertificateThumbprint.Replace(' ', '').ToUpperInvariant()
$certificate = @(
    Get-ChildItem -Path Cert:\CurrentUser\My, Cert:\LocalMachine\My -ErrorAction SilentlyContinue |
        Where-Object { $_.Thumbprint -eq $normalizedThumbprint }
) | Select-Object -First 1
if ($null -eq $certificate) {
    throw "Code-signing certificate '$normalizedThumbprint' was not found."
}
if (-not $certificate.HasPrivateKey) {
    throw "Certificate '$normalizedThumbprint' does not have an accessible private key."
}
$codeSigningOid = '1.3.6.1.5.5.7.3.3'
if (-not @($certificate.EnhancedKeyUsageList | Where-Object { $_.ObjectId.Value -eq $codeSigningOid }).Count) {
    throw "Certificate '$normalizedThumbprint' is not valid for code signing."
}

$manifest = Import-PowerShellDataFile -LiteralPath $ManifestPath
if ($manifest.FormatVersion -ne 1) {
    throw "Unsupported release manifest format '$($manifest.FormatVersion)'."
}
$rootPrefix = $root.TrimEnd([IO.Path]::DirectorySeparatorChar, [IO.Path]::AltDirectorySeparatorChar) +
    [IO.Path]::DirectorySeparatorChar
$signablePaths = foreach ($relativePath in $manifest.Files) {
    if (-not ($relativePath -is [string]) -or [string]::IsNullOrWhiteSpace($relativePath)) {
        throw 'Every release manifest file must be a non-empty string.'
    }
    $archivePath = $relativePath.Replace('\', '/')
    $segments = @($archivePath.Split('/'))
    if (
        [IO.Path]::IsPathRooted($relativePath) -or
        $segments.Count -eq 0 -or
        @($segments | Where-Object { $_ -eq '' -or $_ -eq '.' -or $_ -eq '..' }).Count -gt 0
    ) {
        throw "Release manifest path must be a normalized relative path: '$relativePath'."
    }
    if ([IO.Path]::GetExtension($archivePath) -notin @('.ps1', '.psm1', '.psd1')) {
        continue
    }

    $fullPath = [IO.Path]::GetFullPath(
        (Join-Path $root ($archivePath.Replace('/', [IO.Path]::DirectorySeparatorChar)))
    )
    if (-not $fullPath.StartsWith($rootPrefix, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Release manifest path escapes the repository root: '$relativePath'."
    }
    $fullPath
}
$signablePaths = @($signablePaths)
if ($signablePaths.Count -eq 0) {
    throw 'Release manifest does not contain any Authenticode-signable files.'
}

foreach ($path in $signablePaths) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "Release source file was not found: $path"
    }
    if ($PSCmdlet.ShouldProcess($path, "Sign with certificate $normalizedThumbprint")) {
        $signature = Set-AuthenticodeSignature `
            -LiteralPath $path `
            -Certificate $certificate `
            -HashAlgorithm SHA256 `
            -TimestampServer $TimestampServer `
            -IncludeChain All
        if ($null -eq $signature.SignerCertificate -or $signature.SignerCertificate.Thumbprint -ne $normalizedThumbprint) {
            throw "Authenticode signing did not produce the expected signature for '$path'."
        }
        $signature
    }
}
