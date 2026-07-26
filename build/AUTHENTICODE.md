# Authenticode release signing

Release signing is optional until the repository variable
`REQUIRE_AUTHENTICODE_SIGNATURE` is set to `true`. Once enabled, both test and
release workflows reject an unsigned, untrusted, altered, or expired
non-timestamped PowerShell file listed in `ReleaseFiles.psd1`.

Use a trusted code-signing certificate with an accessible private key:

```powershell
.\build\Set-ReleaseAuthenticodeSignature.ps1 `
    -CertificateThumbprint '<certificate thumbprint>'
```

The helper signs every `.ps1`, `.psm1`, and `.psd1` runtime file in the release
manifest with SHA-256 and uses a timestamp server. Review the resulting source
changes, run the normal test workflow, and commit the signatures before creating
the release tag. Never store a PFX file or its password in the repository.

To verify locally before enabling enforcement:

```powershell
.\build\Build-Release.ps1 -OutputDirectory .\dist
.\build\Test-ReleaseArtifacts.ps1 `
    -ArtifactDirectory .\dist `
    -RequireAuthenticodeSignature
```

The packaged bytes must still exactly match the repository files; signing after
the ZIP is built is therefore not supported.
