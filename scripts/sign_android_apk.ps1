param(
    [Parameter(Mandatory=$true)][string]$Apk,
    [Parameter(Mandatory=$true)][string]$Output,
    [Parameter(Mandatory=$true)][string]$ApkSignerJar,
    [string]$JavaHome = (Join-Path $PSScriptRoot '../.tools/jdk/jdk-17.0.18+8'),
    [string]$SigningDirectory = (Join-Path ([Environment]::GetFolderPath('MyDocuments')) 'Media Manager Signing')
)
$ErrorActionPreference='Stop'
$Apk=(Resolve-Path -LiteralPath $Apk).Path
$ApkSignerJar=(Resolve-Path -LiteralPath $ApkSignerJar).Path
$java=Join-Path (Resolve-Path -LiteralPath $JavaHome).Path 'bin/java.exe'
$keystore=Join-Path $SigningDirectory 'media-manager-upload.p12'
$passwordPath=Join-Path $SigningDirectory 'upload-password.dpapi'
if (!(Test-Path -LiteralPath $keystore) -or !(Test-Path -LiteralPath $passwordPath)) {throw 'Restore the existing upload signing identity before signing. No replacement key will be generated.'}
$Output=[IO.Path]::GetFullPath($Output)
if (Test-Path -LiteralPath $Output) {throw 'Signed output already exists; preserve it or choose a new filename.'}
$null=New-Item -ItemType Directory -Path ([IO.Path]::GetDirectoryName($Output)) -Force
$secure=(Get-Content -LiteralPath $passwordPath -Raw).Trim() | ConvertTo-SecureString
$pointer=[Runtime.InteropServices.Marshal]::SecureStringToBSTR($secure)
try {
    $env:MEDIA_MANAGER_APK_PASSWORD=[Runtime.InteropServices.Marshal]::PtrToStringBSTR($pointer)
    & $java -jar $ApkSignerJar sign --ks $keystore --ks-type PKCS12 --ks-key-alias media-manager-upload --ks-pass env:MEDIA_MANAGER_APK_PASSWORD --key-pass env:MEDIA_MANAGER_APK_PASSWORD --v4-signing-enabled false --out $Output $Apk
    if($LASTEXITCODE -ne 0){throw 'APK signing failed'}
    $verification=& $java -jar $ApkSignerJar verify --verbose --print-certs $Output 2>&1
    $verification | Set-Content -LiteralPath ($Output+'.verification.txt')
    if($LASTEXITCODE -ne 0){throw 'APK signature verification failed'}
    $hash=(Get-FileHash -LiteralPath $Output -Algorithm SHA256).Hash.ToLowerInvariant()
    "$hash  $([IO.Path]::GetFileName($Output))" | Set-Content -LiteralPath ($Output+'.sha256')
    Write-Output "Signed and verified APK: $Output"
} finally {
    Remove-Item Env:MEDIA_MANAGER_APK_PASSWORD -ErrorAction SilentlyContinue
    [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($pointer)
}
