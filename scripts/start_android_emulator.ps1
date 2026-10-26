param([switch]$InstallApp)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
$env:ANDROID_HOME = Join-Path $projectRoot '.tools/android-sdk'
$env:ANDROID_SDK_ROOT = $env:ANDROID_HOME
$env:ANDROID_AVD_HOME = Join-Path $projectRoot '.tools/android-avd'
$env:ANDROID_USER_HOME = Join-Path $projectRoot '.tools/android-user'
$adb = Join-Path $env:ANDROID_HOME 'platform-tools/adb.exe'
$emulator = Join-Path $env:ANDROID_HOME 'emulator/emulator.exe'
if (!(Test-Path $emulator)) { throw 'Install the local Android SDK in .tools/android-sdk first.' }
if (!(& $adb devices | Select-String '^emulator-\d+\s+device')) {
    Start-Process -FilePath $emulator -ArgumentList @('-avd', 'Media_Manager_API_35', '-gpu', 'auto', '-no-snapshot-load')
}
if ($InstallApp) {
    $apk = Join-Path $projectRoot '.tools/mobile-validation/android-evidence/Media-Manager-emulator.apk'
    if (!(Test-Path $apk)) { throw "Validated emulator APK is missing: $apk" }
    $deadline = (Get-Date).AddMinutes(5)
    do {
        Start-Sleep -Seconds 3
        $booted = (& $adb -e shell getprop sys.boot_completed 2>$null) -join ''
        if ((Get-Date) -gt $deadline) { throw 'Android did not finish booting within five minutes.' }
    } until ($booted.Trim() -eq '1')
    & $adb -e install -r $apk
    if ($LASTEXITCODE -ne 0) { throw 'APK installation failed.' }
    & $adb -e shell monkey -p com.tvshowmanager.app -c android.intent.category.LAUNCHER 1
}
