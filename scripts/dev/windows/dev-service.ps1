<#
.SYNOPSIS
  Manage the Warren daemon as a local Windows dev service.

.DESCRIPTION
  On Windows the daemon must run as the SYSTEM user (it edits the WFP firewall
  and creates the WinTUN adapter). Registering it as a service is the clean way
  to get SYSTEM; this script also delegates Start/Stop/Query to interactive
  users (well-known SID `IU`), so once installed (one elevated step) the service
  can be driven without re-elevating.

  Build the daemon first: scripts/dev/windows/build-daemon.sh

.PARAMETER Action
  Install : (re)create the WarrenVPN service as SYSTEM + delegate start/stop (elevates).
  Remove  : stop + delete the service (elevates).
  Start / Stop / Restart / Status / Logs : drive the service (no elevation needed).

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File scripts/dev/windows/dev-service.ps1 -Action Install
  powershell -ExecutionPolicy Bypass -File scripts/dev/windows/dev-service.ps1 -Action Start
#>
param(
    [ValidateSet('Install', 'Remove', 'Start', 'Stop', 'Restart', 'Status', 'Logs')]
    [string]$Action = 'Status'
)
$ErrorActionPreference = 'Stop'

$repo    = (Resolve-Path "$PSScriptRoot\..\..\..").Path
$daemon  = Join-Path $repo 'target\debug\warren-daemon.exe'
$resDir  = Join-Path $repo 'dist-assets'
$logDir  = Join-Path $repo 'dev-logs'
$logFile = Join-Path $logDir 'daemon.log'
$svc     = 'WarrenVPN'

function Test-Admin {
    ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
    ).IsInRole([Security.Principal.WindowsBuiltinRole]::Administrator)
}

function Invoke-Elevated($act) {
    Write-Host "Action '$act' needs admin; requesting elevation (UAC)..." -ForegroundColor Yellow
    Start-Process powershell -Verb RunAs -ArgumentList @(
        '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', "`"$PSCommandPath`"", '-Action', $act
    ) -Wait
}

switch ($Action) {
    'Install' {
        if (-not (Test-Admin)) { Invoke-Elevated 'Install'; break }
        if (-not (Test-Path $daemon)) {
            throw "Daemon not built: $daemon (run scripts/dev/windows/build-daemon.sh first)"
        }
        New-Item -ItemType Directory -Force -Path $logDir | Out-Null
        cmd /c "sc stop $svc" 2>$null | Out-Null
        cmd /c "sc delete $svc" 2>$null | Out-Null
        Start-Sleep -Milliseconds 800

        # The SYSTEM service inherits these from the machine environment at start.
        & setx /M WARREN_RESOURCE_DIR "$resDir" | Out-Null
        & setx /M MULLVAD_LOG_DIR "$logDir" | Out-Null

        $bin = "\`"$daemon\`" --run-as-service -vv"
        cmd /c "sc create $svc binPath= `"$bin`" obj= LocalSystem start= demand DisplayName= `"Warren VPN Service (dev)`""
        if ($LASTEXITCODE -ne 0) { throw "sc create failed ($LASTEXITCODE)" }

        # Grant Interactive Users (IU) start/stop/query so the service can be
        # driven without elevation; SYSTEM (SY) + Administrators (BA) keep full.
        $sddl = 'D:(A;;CCLCSWRPWPDTLOCRRC;;;SY)(A;;CCDCLCSWRPWPDTLOCRSDRCWDWO;;;BA)(A;;CCLCSWRPWPLORC;;;IU)(A;;CCLCSWLOCRRC;;;SU)'
        cmd /c "sc sdset $svc `"$sddl`""
        if ($LASTEXITCODE -ne 0) { throw "sc sdset failed ($LASTEXITCODE)" }
        Write-Host "Installed. Start it (no elevation): dev-service.ps1 -Action Start" -ForegroundColor Green
    }
    'Remove' {
        if (-not (Test-Admin)) { Invoke-Elevated 'Remove'; break }
        cmd /c "sc stop $svc" 2>$null | Out-Null
        Start-Sleep -Milliseconds 500
        cmd /c "sc delete $svc"
        Write-Host "Removed $svc." -ForegroundColor Green
    }
    'Start'   { & sc.exe start $svc }
    'Stop'    { & sc.exe stop $svc }
    'Restart' { & sc.exe stop $svc 2>$null | Out-Null; Start-Sleep -Seconds 2; & sc.exe start $svc }
    'Status'  { & sc.exe query $svc }
    'Logs'    {
        if (Test-Path $logFile) { Get-Content $logFile -Tail 80 }
        else { Write-Host "No log yet at $logFile (is the service started?)" -ForegroundColor Yellow }
    }
}
