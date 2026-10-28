param(
    [string]$Bundle,
    [string]$Output,
    [switch]$InitializeOnly,
    [string]$JavaHome = (Join-Path $PSScriptRoot '../.tools/jdk/jdk-17.0.18+8'),
    [string]$SigningDirectory = (Join-Path ([Environment]::GetFolderPath('MyDocuments')) 'Media Manager Signing')
)
$ErrorActionPreference='Stop'
if(!$InitializeOnly){
    if(!$Bundle -or !$Output){throw "Bundle and Output are required for signing"}
    $Bundle=(Resolve-Path -LiteralPath $Bundle).Path
}
$JavaHome=(Resolve-Path -LiteralPath $JavaHome).Path
$null=New-Item -ItemType Directory -Path $SigningDirectory -Force
$SigningDirectory=(Resolve-Path -LiteralPath $SigningDirectory).Path
$identity=[Security.Principal.WindowsIdentity]::GetCurrent().User
$security=Get-Acl -LiteralPath $SigningDirectory
$rules=$security.GetAccessRules($true,$true,[Security.Principal.SecurityIdentifier])
$private=$security.AreAccessRulesProtected -and $rules.Count -gt 0 -and @($rules | Where-Object {$_.IdentityReference.Value -ne $identity.Value}).Count -eq 0
if(!$private){
    $security=[Security.AccessControl.DirectorySecurity]::new()
    $security.SetAccessRuleProtection($true,$false)
    $security.SetOwner($identity)
    $rule=[Security.AccessControl.FileSystemAccessRule]::new($identity,'FullControl','ContainerInherit,ObjectInherit','None','Allow')
    $security.AddAccessRule($rule)
    Set-Acl -LiteralPath $SigningDirectory -AclObject $security
}
$keystore=Join-Path $SigningDirectory 'media-manager-upload.p12'
$passwordPath=Join-Path $SigningDirectory 'upload-password.dpapi'
$certificate=Join-Path $SigningDirectory 'upload-certificate.pem'
$keytool=Join-Path $JavaHome 'bin/keytool.exe'
$jarsigner=Join-Path $JavaHome 'bin/jarsigner.exe'
if ((Test-Path -LiteralPath $keystore) -and !(Test-Path -LiteralPath $passwordPath)) {throw 'The upload key password file is missing. Recover it before signing.'}
if (!(Test-Path -LiteralPath $keystore) -and (Test-Path -LiteralPath $certificate)) {throw 'An existing public upload certificate has no private key. Restore the original keystore; do not replace it.'}
if (!(Test-Path -LiteralPath $passwordPath)) {
    $bytes=[byte[]]::new(48)
    [Security.Cryptography.RandomNumberGenerator]::Fill($bytes)
    $secret=ConvertTo-SecureString ([Convert]::ToBase64String($bytes)) -AsPlainText -Force
    ConvertFrom-SecureString $secret | Set-Content -LiteralPath $passwordPath
}
$secure=(Get-Content -LiteralPath $passwordPath -Raw).Trim() | ConvertTo-SecureString
$pointer=[Runtime.InteropServices.Marshal]::SecureStringToBSTR($secure)
try {
    $env:MEDIA_MANAGER_UPLOAD_PASSWORD=[Runtime.InteropServices.Marshal]::PtrToStringBSTR($pointer)
    if (!(Test-Path -LiteralPath $keystore)) {
        & $keytool -genkeypair -keystore $keystore -storetype PKCS12 -alias media-manager-upload -keyalg RSA -keysize 3072 -sigalg SHA256withRSA -validity 10000 -dname 'CN=Media Manager Upload,O=Media Manager' -storepass:env MEDIA_MANAGER_UPLOAD_PASSWORD -keypass:env MEDIA_MANAGER_UPLOAD_PASSWORD -noprompt
        if($LASTEXITCODE -ne 0){throw 'Upload key generation failed'}
    }
    & $keytool -exportcert -rfc -keystore $keystore -alias media-manager-upload -storepass:env MEDIA_MANAGER_UPLOAD_PASSWORD -file $certificate
    if($LASTEXITCODE -ne 0){throw 'Could not export upload certificate'}
    if($InitializeOnly){Write-Output "Upload signing identity initialized: $SigningDirectory";return}
    $Output=[IO.Path]::GetFullPath($Output)
    $null=New-Item -ItemType Directory -Path ([IO.Path]::GetDirectoryName($Output)) -Force
    if(Test-Path -LiteralPath $Output){throw 'Signed output already exists; preserve the earlier bundle or choose a new filename.'}
    & $jarsigner -keystore $keystore -storetype PKCS12 -storepass:env MEDIA_MANAGER_UPLOAD_PASSWORD -keypass:env MEDIA_MANAGER_UPLOAD_PASSWORD -sigalg SHA256withRSA -digestalg SHA-256 -signedjar $Output $Bundle media-manager-upload
    if($LASTEXITCODE -ne 0){throw 'AAB signing failed'}
    $verification=& $jarsigner -verify -verbose -certs $Output 2>&1
    $verification | Set-Content -LiteralPath ($Output+'.verification.txt')
    if($LASTEXITCODE -ne 0 -or ($verification -join "`n") -notmatch 'jar verified\.' -or ($verification -join "`n") -match 'jar is unsigned|unsigned entries'){throw 'Signed bundle verification failed'}
    $hash=(Get-FileHash -LiteralPath $Output -Algorithm SHA256).Hash.ToLowerInvariant()
    "$hash  $([IO.Path]::GetFileName($Output))" | Set-Content -LiteralPath ($Output+'.sha256')
    Copy-Item -LiteralPath $certificate -Destination (Join-Path ([IO.Path]::GetDirectoryName($Output)) 'Media-Manager-upload-certificate.pem') -Force
    Write-Output "Signed bundle: $Output"
    Write-Output "Upload key stored privately: $SigningDirectory"
} finally {
    Remove-Item Env:MEDIA_MANAGER_UPLOAD_PASSWORD -ErrorAction SilentlyContinue
    [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($pointer)
}
