#!/usr/bin/env bash
#
# Tests for ci/codemagic/watchdog.sh, the guard every Codemagic step runs
# under. It exists because a build once sat 42 minutes on a process waiting for
# stdin with nothing to show for it: the guard must kill a silent, idle command
# quickly, never kill one that is working, and pass the command's own exit
# status through untouched.
#
# The two Windows-specific halves (the CPU probe and the tree kill) are
# replaced by stand-ins through WATCHDOG_IDLE_PROBE and plain signals, so this
# suite runs on any Unix CI runner in seconds.
set -uo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
guard="$here/watchdog.sh"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
failures=0

printf '#!/bin/sh\nexit 0\n' > "$tmp/idle"
printf '#!/bin/sh\nexit 1\n' > "$tmp/busy"
chmod +x "$tmp/idle" "$tmp/busy"

run() { # run <idle probe> <idle after> <silent max> <command...>
    local probe="$1" idle_after="$2" silent_max="$3"
    shift 3
    local stand_in="$tmp/$probe"
    [ "$probe" = real ] && stand_in=""
    WATCHDOG_TICK=1 WATCHDOG_IDLE_PROBE="$stand_in" \
        WATCHDOG_IDLE_AFTER="$idle_after" WATCHDOG_SILENT_MAX="$silent_max" \
        "$guard" "$@" > "$tmp/out" 2>&1
}

check() { # check <description> <condition>
    if eval "$2"; then
        echo "ok: $1"
    else
        echo "FAIL: $1"
        sed 's/^/    /' "$tmp/out"
        failures=$((failures + 1))
    fi
}

start=$SECONDS
run idle 2 60 sleep 30
rc=$?
check "kills a silent command once the machine is idle" \
    "[ $rc -eq 124 ] && [ $((SECONDS - start)) -lt 15 ] && grep -q 'processes are idle' '$tmp/out'"

start=$SECONDS
run busy 2 60 sh -c 'sleep 6; echo done'
rc=$?
check "spares a silent command while the machine is busy" \
    "[ $rc -eq 0 ] && grep -q '^done$' '$tmp/out'"

start=$SECONDS
run busy 2 4 sleep 30
rc=$?
check "kills a command silent past the hard limit even on a busy machine" \
    "[ $rc -eq 124 ] && [ $((SECONDS - start)) -lt 15 ] && grep -q 'no output for' '$tmp/out'"

run idle 3 60 sh -c 'for i in 1 2 3 4 5 6 7 8; do echo tick $i; sleep 1; done'
rc=$?
check "never kills a command that keeps printing" \
    "[ $rc -eq 0 ] && grep -q '^tick 8$' '$tmp/out'"

run idle 60 60 sh -c 'echo failing; exit 3'
rc=$?
check "passes the command's own exit status through" "[ $rc -eq 3 ]"

# The Codemagic case: a stdin pipe nobody ever closes.
start=$SECONDS
run idle 60 60 sh -c 'read -r line; echo "read returned"' < <(sleep 20)
rc=$?
check "gives the command a closed stdin, whatever it was handed" \
    "[ $rc -eq 0 ] && [ $((SECONDS - start)) -lt 10 ] && grep -q 'read returned' '$tmp/out'"

run idle 2 60 sh -c 'sleep 30 & wait'
rc=$?
check "kills the command's children with it" "[ $rc -eq 124 ]"

run idle 60 60 sh -c 'echo x; sleep 3'
check "reports the longest silence it saw" "grep -q 'longest silence: [0-9]*s' '$tmp/out'"

# A kill that fails to reach part of the tree (a child that ignores the
# signal, or on Windows a native process outside the group) must still end the
# step: the guard never waits on what it killed for more than a moment.
# A kill that fails to reach part of the tree must still end the step: the
# guard never waits on what it killed for more than a moment. WATCHDOG_TEST_KILL
# =none turns the kill into a no-op, the worst case of a tree it cannot reach.
start=$SECONDS
WATCHDOG_TEST_KILL=none run idle 2 60 sleep 120
rc=$?
check "ends the step even when the kill reaches nothing" \
    "[ $rc -eq 124 ] && [ $((SECONDS - start)) -lt 50 ]"
pkill -f 'sleep 120' 2> /dev/null

# The command is over when it exits, not when the last holder of its output
# lets go: a daemon it left behind (MSVC's mspdbsrv.exe is one) must not keep
# the step alive.
start=$SECONDS
run idle 60 60 sh -c 'sleep 40 & echo started'
rc=$?
check "returns when the command exits, whatever it left running" \
    "[ $rc -eq 0 ] && [ $((SECONDS - start)) -lt 15 ] && grep -q '^started$' '$tmp/out'"
pkill -f 'sleep 40' 2> /dev/null

# On the Codemagic machine itself, a native Windows tree must die too.
if case "$(uname -s)" in MINGW* | MSYS*) true ;; *) false ;; esac; then
    start=$SECONDS
    run idle 2 60 powershell.exe -NoProfile -NonInteractive -Command 'Start-Sleep -Seconds 300'
    rc=$?
    check "kills a native Windows process that went silent" \
        "[ $rc -eq 124 ] && [ $((SECONDS - start)) -lt 50 ]"
    start=$SECONDS
    run busy 2 5 cmd.exe //c 'ping -n 300 127.0.0.1 > nul'
    rc=$?
    check "kills a native Windows tree past the hard limit" \
        "[ $rc -eq 124 ] && [ $((SECONDS - start)) -lt 50 ]"

    # The real CPU probe, on the command's own tree: a busy machine must not
    # hide a hung command, and a silent command that computes must not read
    # as hung. The burner runs outside the guarded tree.
    powershell.exe -NoProfile -NonInteractive -Command 'while ($true) { }' < /dev/null > /dev/null 2>&1 &
    burner=$!
    start=$SECONDS
    run real 20 300 powershell.exe -NoProfile -NonInteractive -Command 'Start-Sleep -Seconds 300'
    rc=$?
    check "kills a silent native command while the rest of the machine is busy" \
        "[ $rc -eq 124 ] && [ $((SECONDS - start)) -lt 90 ] && grep -q 'processes are idle' '$tmp/out'"
    taskkill //T //F //PID "$(cat "/proc/$burner/winpid")" > /dev/null 2>&1
    # The build's real shape: the work runs several MSYS generations below
    # the guarded command (bash, build.sh, cargo, rustc, link.exe). A tree
    # walk that stops at the first generation reads that work as idle: it
    # killed a release build in the middle of its LTO link once.
    printf '%s\n' '$end = (Get-Date).AddSeconds(60); while ((Get-Date) -lt $end) { }' > "$tmp/busy.ps1"
    printf 'powershell.exe -NoProfile -NonInteractive -File "%s"\n' "$(cygpath -w "$tmp/busy.ps1")" > "$tmp/inner.sh"
    printf 'bash "%s"\n' "$tmp/inner.sh" > "$tmp/middle.sh"
    run real 20 300 bash "$tmp/middle.sh"
    rc=$?
    check "spares a silent computation deep in an MSYS process tree" "[ $rc -eq 0 ]"
    printf '%s\n' 'Start-Sleep -Seconds 300' > "$tmp/hang.ps1"
    printf 'powershell.exe -NoProfile -NonInteractive -File "%s"\n' "$(cygpath -w "$tmp/hang.ps1")" > "$tmp/inner-hang.sh"
    printf 'bash "%s"\n' "$tmp/inner-hang.sh" > "$tmp/middle-hang.sh"
    start=$SECONDS
    run real 20 300 bash "$tmp/middle-hang.sh"
    rc=$?
    check "kills a silent hang deep in an MSYS process tree" \
        "[ $rc -eq 124 ] && [ $((SECONDS - start)) -lt 90 ] && grep -q 'processes are idle' '$tmp/out'"
    run real 20 300 powershell.exe -NoProfile -NonInteractive -Command '$end = (Get-Date).AddSeconds(60); while ((Get-Date) -lt $end) { }'
    rc=$?
    check "spares a silent native command that is computing" "[ $rc -eq 0 ]"
fi

# The guard only protects the steps that go through it: every Git Bash step of
# this repository's codemagic.yaml must run under watchdog.sh.
root="$here"
while [ "$root" != / ] && [ ! -f "$root/codemagic.yaml" ]; do root="$(dirname "$root")"; done
yaml="$root/codemagic.yaml"
steps="$(grep -c "bash.exe'" "$yaml")"
unguarded="$(grep "bash.exe'" "$yaml" | grep -vc 'codemagic/watchdog.sh ')"
{ echo "codemagic.yaml: $steps Git Bash step(s)"; grep "bash.exe'" "$yaml" | grep -v 'codemagic/watchdog.sh '; } > "$tmp/out"
check "every Git Bash step of codemagic.yaml runs under the watchdog" \
    "[ $steps -gt 0 ] && [ $unguarded -eq 0 ]"

[ "$failures" -eq 0 ] || { echo "$failures failure(s)"; exit 1; }
echo "all watchdog tests passed"
