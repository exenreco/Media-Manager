# For disposable CI runners only. Does not use a user's normal installation path.
$ErrorActionPreference = 'Stop'
$taskInstaller = (Get-Item 'dist/*-setup.exe').FullName
$taskInstallDirectory = Join-Path $PWD '.tools/installer-smoke'
foreach ($taskAttempt in 1..2) {
    $taskSetup = Start-Process -FilePath $taskInstaller -ArgumentList @('/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART',('/DIR="' + $taskInstallDirectory + '"')) -PassThru -Wait -WindowStyle Hidden
    if ($taskSetup.ExitCode -ne 0) { throw "Installer failed: $($taskSetup.ExitCode)" }
}
foreach ($taskName in @('Media-Manager.exe','media-manager-server.exe','unins000.exe')) {
    if (-not (Test-Path -LiteralPath (Join-Path $taskInstallDirectory $taskName))) { throw "Missing installed file: $taskName" }
}
python scripts/test_installed_app.py (Join-Path $taskInstallDirectory 'Media-Manager.exe')
if ($LASTEXITCODE -ne 0) { throw 'Installed app launch failed' }
$taskUninstall = Start-Process -FilePath (Join-Path $taskInstallDirectory 'unins000.exe') -ArgumentList '/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART' -PassThru -Wait -WindowStyle Hidden
if ($taskUninstall.ExitCode -ne 0) { throw "Uninstall failed: $($taskUninstall.ExitCode)" }
if (Test-Path -LiteralPath (Join-Path $taskInstallDirectory 'Media-Manager.exe')) { throw 'Uninstall left the app executable behind' }
