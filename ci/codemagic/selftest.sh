#!/usr/bin/env bash
#
# Checks, on a real Windows machine, what the Unix CI cannot. warren-tests.yml
# runs the watchdog, powershell and prologue phases on windows-2025; run the
# others by hand when a Windows step misbehaves.
#
#   ci/codemagic/selftest.sh <watchdog|idle-cpu|powershell|tree|prologue>
set -euo pipefail
exec < /dev/null

case "${1:?usage: selftest.sh <watchdog|idle-cpu|powershell|tree|prologue>}" in
    watchdog)
        # Includes the Windows-only cases: native process trees, and the CPU
        # of the command's own tree while the rest of the machine is busy.
        bash ci/codemagic/test-watchdog.sh
        ;;
    idle-cpu)
        # What "idle" reads on this machine, background work included.
        for round in 1 2 3; do
            MSYS_NO_PATHCONV=1 typeperf '\Processor(_Total)\% Processor Time' -si 5 -sc 4 \
                | tr -d '\r"' | awk -F, -v r="$round" '$2 ~ /^[0-9.]+$/ { s += $2; n++ } END { printf "idle round %d: %.1f%% average CPU\n", r, s / n }'
        done
        ;;
    powershell)
        # Regression check for the hang Codemagic's PowerShell profile causes
        # (ci/codemagic/windows-env.sh, neutralize_powershell_profile): once
        # the build environment is entered, a PowerShell started the way the
        # build's tools start it (profile loading allowed) and captured must
        # return. Guarded to a minute.
        # shellcheck source=ci/codemagic/windows-env.sh
        source ci/codemagic/windows-env.sh
        WATCHDOG_SILENT_MAX=60 WATCHDOG_TICK=5 bash ci/codemagic/watchdog.sh \
            bash -c 'out="$(powershell.exe -Command "Write-Output ok")"; [ "$(printf %s "$out" | tr -d "\r")" = ok ] && echo "captured: $out"'
        ;;
    tree)
        # How an MSYS chain maps onto Windows parent ids, which the watchdog's
        # CPU reading walks: bash -> bash -> powershell (busy for 30 s).
        work="$(mktemp -d)"
        printf '%s\n' '$end = (Get-Date).AddSeconds(30); while ((Get-Date) -lt $end) { }' > "$work/busy.ps1"
        printf 'powershell.exe -NoProfile -NonInteractive -File "%s"\n' "$(cygpath -w "$work/busy.ps1")" > "$work/inner.sh"
        printf 'bash "%s"\n' "$work/inner.sh" > "$work/middle.sh"
        bash "$work/middle.sh" &
        root=$!
        sleep 8
        echo "root msys pid $root, winpid $(cat "/proc/$root/winpid")"
        ps -W 2> /dev/null | grep -Ei 'bash|powershell' || ps
        powershell.exe -NoProfile -NonInteractive -Command 'Get-CimInstance Win32_Process | Where-Object { $_.Name -match (Write-Output bash.exe powershell.exe | Out-String).Trim().Replace([Environment]::NewLine, [string][char]124) } | Select-Object ProcessId,ParentProcessId,Name,KernelModeTime,UserModeTime | Format-Table -AutoSize | Out-String -Width 200'
        wait "$root"
        ;;
    prologue)
        # What build.sh runs before its first line of output, which once hung
        # for good on this machine (scripts/utils/host's former PowerShell
        # probe). Guarded to one minute: a regression fails here, fast.
        WATCHDOG_SILENT_MAX=60 WATCHDOG_TICK=5 bash ci/codemagic/watchdog.sh \
            bash -c 'source scripts/utils/host && echo "HOST=$HOST"'
        ;;
    *) echo "unknown phase: $1" >&2; exit 2 ;;
esac
