param([int]$BackendPort = 8787)
$ErrorActionPreference = 'Stop'
if ($BackendPort -lt 1 -or $BackendPort -gt 65535) { throw 'Invalid backend port.' }
$projectRoot = Split-Path $PSScriptRoot -Parent
$runtime = Join-Path $projectRoot '.tools/https'
$client = Join-Path $runtime 'cloudflared.exe'
if (!(Test-Path -LiteralPath $client)) { throw 'Install the verified Cloudflare client in .tools/https/cloudflared.exe first.' }
if (!(Get-NetTCPConnection -LocalPort $BackendPort -State Listen -ErrorAction SilentlyContinue | Where-Object LocalAddress -eq '127.0.0.1')) {
    throw 'Start the authenticated Media Manager backend on 127.0.0.1 first.'
}
# Creates a public temporary HTTPS endpoint. Backend authentication is still required.
$process = Start-Process -FilePath $client -ArgumentList @('tunnel', '--no-autoupdate', '--url', "http://127.0.0.1:$BackendPort") `
    -WindowStyle Hidden -RedirectStandardOutput (Join-Path $runtime 'tunnel-output.log') `
    -RedirectStandardError (Join-Path $runtime 'tunnel.log') -PassThru
Write-Output "HTTPS tunnel process: $($process.Id). Stop it with Stop-Process -Id $($process.Id)."
$deadline = (Get-Date).AddSeconds(45)
do {
    Start-Sleep -Seconds 1
    $process.Refresh()
    $log = Get-Content -LiteralPath (Join-Path $runtime 'tunnel.log') -Raw -ErrorAction SilentlyContinue
    if ($process.HasExited) { throw "HTTPS tunnel failed. Review $(Join-Path $runtime 'tunnel.log'). No working endpoint was confirmed." }
    if ($log -match 'https://[a-z0-9-]+\.trycloudflare\.com') {
        Write-Output "Temporary HTTPS address: $($Matches[0])"
        Write-Output 'Verify the authenticated health endpoint before pairing. The address changes when restarted.'
        return
    }
} while ((Get-Date) -lt $deadline)
throw "No HTTPS address was assigned within 45 seconds. Review $(Join-Path $runtime 'tunnel.log'); process $($process.Id) may still be running."
