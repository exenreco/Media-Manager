param([string]$Binaries = 'target/release')
$ErrorActionPreference = 'Stop'
# Run after signing and before packaging/checksum generation. Both executables
# must be signed and timestamped by a certificate trusted on the build host.
foreach ($taskBinary in @('tv-manager-app.exe', 'tv-manager-server.exe')) {
    $taskPath = Join-Path $Binaries $taskBinary
    $taskSignature = Get-AuthenticodeSignature -LiteralPath $taskPath
    if ($taskSignature.Status -ne 'Valid' -or $null -eq $taskSignature.TimeStamperCertificate) {
        throw "Missing, invalid or untimestamped Authenticode signature: $taskBinary"
    }
    Write-Output "$taskBinary verified: $($taskSignature.SignerCertificate.Subject)"
}
