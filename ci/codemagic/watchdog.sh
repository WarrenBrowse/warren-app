#!/usr/bin/env bash
#
# Run a command and kill it when it stops making progress.
#
#   ci/codemagic/watchdog.sh <command> [args...]
#
# Every Codemagic step runs under this guard. Codemagic only enforces the
# workflow's max_build_duration (up to 120 minutes) and cannot show a running
# step's log, so a hung command would otherwise burn the whole budget in
# silence: the first release build sat 42 minutes on a PowerShell child
# waiting for stdin before anyone noticed.
#
# Progress is output. The command is killed, with its whole process tree, when
#   - it printed nothing for WATCHDOG_IDLE_AFTER seconds (default 300) and its
#     own processes used less than WATCHDOG_IDLE_CPU CPU-seconds (default 5)
#     since the first fifth of that silence: waiting on stdin, a lock or a
#     stalled download; or
#   - it printed nothing for WATCHDOG_SILENT_MAX seconds (default 1800) at
#     all: a loop that burns CPU without progressing.
# The CPU is the command's own process tree, never the machine's: an idle
# Codemagic windows_x2 machine read anywhere from 0.6 % to 40 % across two
# self-tests of 2026-09-26, background work included, so a machine-wide
# reading cannot tell a hang from work. A silent phase that computes, such as
# the release profile's fat LTO link (one core, 30 CPU-seconds every 30 s),
# is never idle. The longest silence of every run is printed, so the hard
# limit can be tightened from measurements.
#
# Killed: exit 124 with a ::error line naming the reason. Otherwise the
# command's own exit status. The command gets a closed stdin.
#
# Canonical copy: warren-app ci/codemagic/watchdog.sh, tested by
# ci/codemagic/test-watchdog.sh. warren-sdk-rs, warren-sdk-ts and wclaude carry
# byte-identical copies under scripts/ci/codemagic/; change them together.
set -uo pipefail
exec < /dev/null

[ $# -gt 0 ] || { echo "usage: watchdog.sh <command> [args...]" >&2; exit 2; }

IDLE_AFTER="${WATCHDOG_IDLE_AFTER:-300}"
SILENT_MAX="${WATCHDOG_SILENT_MAX:-1800}"
IDLE_CPU="${WATCHDOG_IDLE_CPU:-5}"
TICK="${WATCHDOG_TICK:-30}"

work="$(mktemp -d)"
stamp="$work/last-output"
rcfile="$work/rc"
trap 'rm -rf "$work"' EXIT

now() { date +%s; }
last_output() { date -r "$stamp" +%s; }

on_windows() {
    case "$(uname -s)" in MINGW* | MSYS*) return 0 ;; esac
    return 1
}

# Windows pids of <pid> and of every MSYS process below it, through MSYS's own
# parent links. Windows' parent ids cannot be walked across MSYS: each exec
# starts a new Windows process whose parent is the image it replaced, gone by
# then (measured: a Windows-only walk from a bash running a busy build found
# no descendant at all, and the step was killed in the middle of its link).
msys_winpids() { # msys_winpids <msys pid>
    ps -e 2> /dev/null | awk -v root="$1" '
        NR > 1 {
            if ($1 !~ /^[0-9]+$/) { $1 = ""; $0 = $0 }
            parent[$1] = $2; win[$1] = $4; pids[++n] = $1
        }
        END {
            keep[root] = 1
            do {
                grown = 0
                for (i = 1; i <= n; i++) {
                    p = pids[i]
                    if (!(p in keep) && (parent[p] in keep)) { keep[p] = 1; grown = 1 }
                }
            } while (grown)
            for (p in keep) if (p in win) print win[p]
        }'
}

# "<CPU milliseconds> <process count>" of <pid>'s whole tree: the Windows
# processes of its MSYS subtree, and every native process below any of them
# (Windows parent ids hold between native processes: cargo, rustc, link.exe).
# The PowerShell script carries no double quote: Windows PowerShell 5.1 strips
# them from a native command's arguments.
tree_cpu_ms() { # tree_cpu_ms <msys pid>
    local roots
    roots="$(msys_winpids "$1" | awk 'NF { printf "%s%s = $true", (n++ ? "; " : ""), $1 }')"
    [ -n "$roots" ] || return 1
    powershell.exe -NoProfile -NonInteractive -Command "\$all = Get-CimInstance Win32_Process -Property ProcessId,ParentProcessId,KernelModeTime,UserModeTime; \$in = @{ $roots }; do { \$added = 0; foreach (\$p in \$all) { if (\$in.ContainsKey([int]\$p.ParentProcessId) -and -not \$in.ContainsKey([int]\$p.ProcessId)) { \$in[[int]\$p.ProcessId] = \$true; \$added++ } } } while (\$added); \$t = 0; \$n = 0; foreach (\$p in \$all) { if (\$in.ContainsKey([int]\$p.ProcessId)) { \$t += \$p.KernelModeTime + \$p.UserModeTime; \$n++ } }; Write-Output ([string][int64](\$t / 10000) + ' ' + \$n)" < /dev/null 2> /dev/null | tr -d '\r'
}

# Called on every tick of a silence. Exit 0 once the silence is long enough and
# the command's processes did (next to) nothing through it; 1 otherwise, or
# when the CPU cannot be measured (then only the hard limit applies). Only the
# increases between two readings count: a process of the tree that exits takes
# its CPU time out of the sum, and that drop is not idleness (measured: a
# busy loop read 52 s of work, then -7 s the moment it ended).
cpu_prev="" cpu_work=0
command_idle() { # command_idle <silent seconds>
    if [ -n "${WATCHDOG_IDLE_PROBE:-}" ]; then
        [ "$1" -ge "$IDLE_AFTER" ] && "$WATCHDOG_IDLE_PROBE"
        return
    fi
    on_windows || return 1
    [ "$1" -ge $((IDLE_AFTER / 5)) ] || return 1
    local reading used count
    reading="$(tree_cpu_ms "$cmd")"
    if ! printf '%s' "$reading" | grep -Eq '^[0-9]+ [0-9]+$'; then
        echo "watchdog: CPU of the command unreadable, only the ${SILENT_MAX}s limit applies"
        return 1
    fi
    used="${reading% *}" count="${reading#* }"
    if [ -n "$cpu_prev" ] && [ "$used" -gt "$cpu_prev" ]; then
        cpu_work=$((cpu_work + used - cpu_prev))
    fi
    cpu_prev="$used"
    [ "$1" -ge "$IDLE_AFTER" ] || return 1
    echo "watchdog: silent for $1s, the command's $count processes worked $((cpu_work / 1000))s of CPU through it"
    [ "$cpu_work" -lt $((IDLE_CPU * 1000)) ]
}

# Kill <pid> and every process descended from it. No process groups: under
# MSYS without a terminal they are not reliable, and a native Windows child is
# only reachable through its Windows parent chain.
kill_tree() { # kill_tree <pid>
    [ "${WATCHDOG_TEST_KILL:-}" = none ] && return 0
    if on_windows && [ -r "/proc/$1/winpid" ]; then
        taskkill //T //F //PID "$(cat "/proc/$1/winpid")" > /dev/null 2>&1
    fi
    local child
    for child in $(ps -A -o pid= -o ppid= 2> /dev/null | awk -v p="$1" '$2 == p { print $1 }'); do
        kill_tree "$child"
    done
    kill -KILL "$1" 2> /dev/null
}

# Wait up to <seconds> for <pid> to end; 0 when it did.
wait_for() { # wait_for <pid> <seconds>
    local left="$2"
    while kill -0 "$1" 2> /dev/null; do
        [ "$left" -gt 0 ] || return 1
        sleep 1
        left=$((left - 1))
    done
}

: > "$stamp"
# The command runs as a child of the producer subshell, which records its pid
# and then its exit status: the step is over when the command exits, not when
# the last process holding its output lets go of the pipe.
{
    "$@" &
    echo $! > "$work/pid"
    wait $!
    echo $? > "$rcfile"
} 2>&1 | while IFS= read -r line || [ -n "$line" ]; do
    printf '%s\n' "$line"
    : > "$stamp"
done &
reader=$!
while [ ! -s "$work/pid" ]; do sleep 0.1; done
cmd="$(cat "$work/pid")"

reason="" longest=0 ticks=0
while [ ! -s "$rcfile" ]; do
    sleep 1
    ticks=$((ticks + 1))
    [ $((ticks % TICK)) -eq 0 ] || continue
    silent=$(($(now) - $(last_output)))
    [ "$silent" -gt "$longest" ] && longest=$silent
    # A new line ends the silence, and the CPU count with it.
    if [ "$silent" -lt $((IDLE_AFTER / 5)) ]; then
        cpu_prev="" cpu_work=0
    fi
    if [ "$silent" -ge "$SILENT_MAX" ]; then
        reason="no output for ${silent}s (hard limit ${SILENT_MAX}s)"
    elif command_idle "$silent"; then
        reason="no output for ${silent}s and the command's processes are idle"
    fi
    # A command that has just exited is over, not hung.
    [ -s "$rcfile" ] && reason=""
    [ -z "$reason" ] || break
done

if [ -n "$reason" ]; then
    echo "watchdog: longest silence: ${longest}s"
    echo "::error title=watchdog::killed '$*': $reason"
    kill_tree "$cmd"
    # Never wait on what was killed for more than a moment: whatever the kill
    # could not reach, the step ends now.
    wait_for "$reader" 30 || kill -KILL "$reader" 2> /dev/null
    exit 124
fi

# Let the reader print what the command wrote last, but not wait on a
# process the command left behind holding the pipe.
wait_for "$reader" 5 || kill -KILL "$reader" 2> /dev/null
silent=$(($(now) - $(last_output)))
[ "$silent" -gt "$longest" ] && longest=$silent
echo "watchdog: longest silence: ${longest}s"
exit "$(cat "$rcfile")"
