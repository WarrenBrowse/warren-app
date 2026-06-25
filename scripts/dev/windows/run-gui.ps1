<#
.SYNOPSIS
  Launch the Warren Electron app in dev mode (hot-reload) on Windows.

.DESCRIPTION
  Starts `npm run -w mullvad-vpn develop` (Vite dev server + Electron) detached
  in the current interactive desktop session, logging to dev-gui.log at the repo
  root. Chromium only renders in an interactive session, so this must be run from
  the logged-on user's session, NOT from a background/non-interactive context.

  The app connects to the daemon over its named pipe, so start the daemon first
  (scripts/dev/windows/dev-service.ps1 -Action Start).

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File scripts/dev/windows/run-gui.ps1
#>
$ErrorActionPreference = 'Stop'

$repo    = (Resolve-Path "$PSScriptRoot\..\..\..").Path
$desktop = Join-Path $repo 'desktop'
$log     = Join-Path $repo 'dev-gui.log'

if (-not (Test-Path (Join-Path $desktop 'node_modules'))) {
    Write-Host "desktop/node_modules missing; running 'npm run ci' first..." -ForegroundColor Yellow
    Push-Location $desktop
    & npm run ci
    Pop-Location
}

'' | Set-Content $log
Start-Process -FilePath 'cmd.exe' -WindowStyle Hidden -ArgumentList '/c', (
    "cd /d `"$desktop`" && npm run -w mullvad-vpn develop > `"$log`" 2>&1"
)
Write-Host "Electron dev launched (session $((Get-Process -Id $PID).SessionId)). Log: $log" -ForegroundColor Green
Write-Host "Tail it with: Get-Content '$log' -Wait -Tail 40"
