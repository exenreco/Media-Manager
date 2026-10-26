param(
    [Parameter(Mandatory)][string]$InputDirectory,
    [string]$OutputDirectory = 'dist/windows-self-signed',
    [string]$CertificateThumbprint,
    [string]$SignToolPath,
    [uri]$TimestampUrl = 'http://timestamp.digicert.com'
)
$ErrorActionPreference = 'Stop'
if (Test-Path -LiteralPath $OutputDirectory) { throw 'Use a fresh output directory; existing builds will not be overwritten.' }
$taskInput = (Resolve-Path -LiteralPath $InputDirectory).Path
$taskApp = @('Media-Manager.exe', 'tv-manager-app.exe') | Where-Object { Test-Path -LiteralPath (Join-Path $taskInput $_) }
$taskServer = @('media-manager-server.exe', 'tv-manager-server.exe') | Where-Object { Test-Path -LiteralPath (Join-Path $taskInput $_) }
if (@($taskApp).Count -ne 1 -or @($taskServer).Count -ne 1) { throw 'Input must contain exactly one app executable and one server executable.' }
if (!$SignToolPath) {
    $taskSdk = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits/10/bin'
    $SignToolPath = Get-ChildItem -Path "$taskSdk/*/x64/signtool.exe" -ErrorAction SilentlyContinue |
        Sort-Object { [version]$_.Directory.Parent.Name } -Descending | Select-Object -First 1 -ExpandProperty FullName
}
if (!$SignToolPath -or !(Test-Path -LiteralPath $SignToolPath)) { throw 'Install Windows SDK SignTool or supply -SignToolPath.' }
if ($CertificateThumbprint) {
    if ($CertificateThumbprint -notmatch '^[A-Fa-f0-9]{40}$') { throw 'Invalid certificate thumbprint.' }
    $taskCertificate = Get-Item -LiteralPath "Cert:\CurrentUser\My\$CertificateThumbprint"
    if (!$taskCertificate.HasPrivateKey -or $taskCertificate.Subject -ne $taskCertificate.Issuer) { throw 'Use a self-signed test certificate with its private key.' }
} else {
    # CurrentUser\My needs no administrator rights. Do not add this certificate
    # to Root or TrustedPublisher, and do not export its private key.
    $taskCertificate = New-SelfSignedCertificate -Type CodeSigningCert `
        -Subject 'CN=Media Manager Local Test' -FriendlyName 'Media Manager local test signing' `
        -CertStoreLocation 'Cert:\CurrentUser\My' -KeyAlgorithm RSA -KeyLength 3072 `
        -HashAlgorithm SHA256 -KeyExportPolicy NonExportable -NotAfter (Get-Date).AddYears(1)
}
$taskOutput = (New-Item -ItemType Directory -Path $OutputDirectory).FullName
$taskRecords = @()
foreach ($taskName in @($taskApp) + @($taskServer)) {
    $taskSource = Join-Path $taskInput $taskName
    $taskDestination = Join-Path $taskOutput $taskName
    Copy-Item -LiteralPath $taskSource -Destination $taskDestination
    & $SignToolPath sign /sha1 $taskCertificate.Thumbprint /s My /fd SHA256 /tr $TimestampUrl.AbsoluteUri /td SHA256 /d 'Media Manager - local test build' $taskDestination
    if ($LASTEXITCODE -ne 0) { throw "SignTool signing failed for $taskName" }
    $taskSignature = Get-AuthenticodeSignature -LiteralPath $taskDestination
    if ($taskSignature.SignerCertificate.Thumbprint -ne $taskCertificate.Thumbprint -or $null -eq $taskSignature.TimeStamperCertificate) {
        throw "Signer or timestamp verification failed for $taskName"
    }
    $taskVerification = (& $SignToolPath verify /pa /v $taskDestination 2>&1 | Out-String)
    $taskVerifyExit = $LASTEXITCODE
    # An untrusted-root result is expected. Reject other verification failures,
    # including altered file hashes; do not weaken the production signing gate.
    $taskUntrustedRoot = $taskVerification -match '0x800B0109|(?s)terminated in a root\s+certificate which is not trusted'
    if ($taskVerifyExit -ne 0 -and !$taskUntrustedRoot) { throw $taskVerification }
    if ($taskSignature.Status -notin @('Valid', 'NotTrusted', 'UnknownError')) { throw "Unexpected signature state: $($taskSignature.Status)" }
    Set-Content -LiteralPath (Join-Path $taskOutput "$taskName.verification.txt") -Value $taskVerification -Encoding utf8
    $taskRecords += [ordered]@{
        file = $taskName
        sha256 = (Get-FileHash -LiteralPath $taskDestination -Algorithm SHA256).Hash.ToLowerInvariant()
        signerThumbprint = $taskCertificate.Thumbprint
        windowsTrustStatus = $taskSignature.Status.ToString()
        timestamped = $true
    }
}
Export-Certificate -Cert $taskCertificate -FilePath (Join-Path $taskOutput 'Media-Manager-Local-Test.cer') | Out-Null
$taskRecords | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $taskOutput 'signing-report.json') -Encoding utf8
@'
SELF-SIGNED LOCAL TEST BUILD
These files are signed with a test certificate, not a publicly trusted publisher identity.
Windows may display Unknown Publisher or security warnings. Trusted roots were not modified.
The .cer contains only the public certificate. The private key remains non-exportable
in the signing user's CurrentUser\My certificate store. No PFX or password is required.
These test builds are separate from production release assets.
'@ | Set-Content -LiteralPath (Join-Path $taskOutput 'SELF-SIGNED-TEST-BUILD.txt') -Encoding utf8
Write-Output "Self-signed test copies: $taskOutput"
Write-Output "Test certificate thumbprint: $($taskCertificate.Thumbprint)"
