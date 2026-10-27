$ErrorActionPreference = 'Stop'
$taskCompiler = Join-Path ${env:ProgramFiles(x86)} 'Inno Setup 6/ISCC.exe'
if (-not (Test-Path -LiteralPath $taskCompiler)) {
    choco install innosetup --version=6.4.3 --yes --no-progress
    if ($LASTEXITCODE -ne 0) { throw 'Inno Setup installation failed' }
}
$taskDirectory = Join-Path $PWD '.tools/installer'
New-Item -ItemType Directory -Path $taskDirectory -Force | Out-Null
$taskBootstrapper = Join-Path $taskDirectory 'MicrosoftEdgeWebview2Setup.exe'
Invoke-WebRequest -Uri 'https://go.microsoft.com/fwlink/p/?LinkId=2124703' -OutFile $taskBootstrapper
$taskSignature = Get-AuthenticodeSignature -LiteralPath $taskBootstrapper
if ($taskSignature.Status -ne 'Valid' -or $taskSignature.SignerCertificate.Subject -notmatch 'O=Microsoft Corporation(?:,|$)') {
    throw 'WebView2 bootstrapper must have a valid Microsoft Authenticode signature'
}
Write-Output 'Inno Setup and verified Microsoft WebView2 bootstrapper ready.'
