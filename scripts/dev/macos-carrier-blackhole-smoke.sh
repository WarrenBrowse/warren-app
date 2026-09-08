#!/usr/bin/env bash
# Prove that a black-holed macOS carrier bind ends its session at once, on a
# REAL exit, instead of leaving the host dark while two watchdogs count to
# fifteen twice over.
#
# Why this exists: on a multi-interface macOS host an `IP_BOUND_IF`-bound
# carrier socket loses all egress the moment the default route swaps onto the
# TUN. The egress guard measures that within about two seconds and records the
# `<carrier_ip>/32` escape for the network, and until 2026-09-08 it then did
# nothing with the verdict: the session was left to the supervisor's 15 s
# zero-downlink redial (which reuses the same dead bind) and then to the 15 s
# session-liveness deadline. Thirty-one seconds of host-wide blackout, with the
# default route in the tunnel and the kill switch up, on a verdict the app held
# at T + 1.7 s. The guard now ends the session as soon as it measures it.
#
# The decision and the channel are unit-tested. Only a real connect proves the
# whole chain: verdict, cache write, escalation, state machine, and a reconnect
# that picks the escape. This script is that gate, and it must be run on a host
# where the bind actually black-holes (poka's Mac does, on every network whose
# verdict is not already cached).
#
# Run it as root, because it has to flush pf and drive launchd:
#
#   sudo scripts/dev/macos-carrier-blackhole-smoke.sh --beta
#
# Safety, in the order it matters:
#   - A detached, root-owned DEADMAN is armed BEFORE anything is touched. It
#     depends on neither this script nor the daemon: it flushes pf, removes any
#     leftover tunnel default route, restores DNS and brings the installed
#     daemon back, then exits. It is one-shot and re-armed per attempt.
#   - The installed daemon is stopped for the duration, so this never runs two
#     Warren daemons at once (2026-09-01: one agent connecting a second daemon
#     walled this Mac for 11 hours).
#   - Every step is undone on exit, including a normal failure, a Ctrl-C and a
#     SIGKILL of this script.
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=../utils/product-env.sh
source "$REPO_ROOT/scripts/utils/product-env.sh"

usage() {
    cat <<EOF
Real-exit gate for the macOS carrier-blackhole reconnect.

Usage: sudo $(basename "$0") <--prod|--beta|--staging>

Env overrides:
  DEADMAN_SECONDS   deadman horizon (default 180)
  MAX_RECOVER_SECS  how long the whole blackout may last (default 8)
EOF
}

ENV_FLAG=""
for arg in "$@"; do
    if warren_env_flag "$arg"; then ENV_FLAG="$WARREN_ENV_FLAG"; continue; fi
    case "$arg" in
        -h|--help) usage; exit 0 ;;
        *) usage >&2; printf '\nerror: unknown argument: %s\n' "$arg" >&2; exit 1 ;;
    esac
done
warren_env_require "$ENV_FLAG"

[ "$(id -u)" -eq 0 ] || { usage >&2; printf '\nerror: run under sudo\n' >&2; exit 1; }
[ "$(uname -s)" = "Darwin" ] || { printf 'error: macOS only\n' >&2; exit 1; }

PRODUCT_DIR="$(warren_env_product_dir "$WARREN_PRODUCT_ENV")"
DAEMON="$REPO_ROOT/target/release/warren-daemon"
CLI="$REPO_ROOT/target/release/$(warren_env_cli_name "$WARREN_PRODUCT_ENV")"
[ -x "$CLI" ] || CLI="/usr/local/bin/$(warren_env_cli_name "$WARREN_PRODUCT_ENV")"
LOG="/var/log/$PRODUCT_DIR/daemon.log"
CACHE="/Library/Caches/$PRODUCT_DIR/carrier-egress-verdicts.v2"
PLIST="system/com.warrenbrowse.vpn$( [ "$WARREN_PRODUCT_ENV" = prod ] && echo "" || echo ".$WARREN_PRODUCT_ENV" ).daemon"
DEADMAN_SECONDS="${DEADMAN_SECONDS:-180}"
MAX_RECOVER_SECS="${MAX_RECOVER_SECS:-8}"
export WARREN_RPC_SOCKET_PATH="/var/run/$PRODUCT_DIR"

say() { printf '%s\n' "$*"; }
FAIL=0

[ -x "$DAEMON" ] || { say "error: build it first: cargo build --release -p mullvad-daemon"; exit 1; }

# ---------------------------------------------------------------- the deadman
# Detached and root-owned, so it survives this script being SIGKILLed, and it
# undoes the two things that can leave the host dark on their own: a pf ruleset
# that fails closed, and a default route pointing at a TUN nobody owns.
DEADMAN_FLAG="$(mktemp /tmp/warren-blackhole-smoke-armed.XXXXXX)"
arm_deadman() {
    setsid nohup bash -c '
        flag="$1"; horizon="$2"; plist="$3"
        for _ in $(seq 1 "$horizon"); do
            sleep 1
            [ -f "$flag" ] || exit 0
        done
        pkill -TERM -f "target/release/warren-daemon" 2>/dev/null
        sleep 3
        pkill -KILL -f "target/release/warren-daemon" 2>/dev/null
        pfctl -F all 2>/dev/null
        pfctl -d 2>/dev/null
        while route -n delete -inet default -interface utun0 >/dev/null 2>&1; do :; done
        for i in 1 2 3 4 5 6 7 8; do
            route -n delete -inet default -interface "utun$i" >/dev/null 2>&1
        done
        for svc in $(networksetup -listallnetworkservices | tail -n +2); do
            networksetup -setdnsservers "$svc" Empty 2>/dev/null
        done
        dscacheutil -flushcache 2>/dev/null
        killall -HUP mDNSResponder 2>/dev/null
        launchctl kickstart -k "$plist" 2>/dev/null
        rm -f "$flag"
    ' _ "$DEADMAN_FLAG" "$DEADMAN_SECONDS" "$PLIST" >/dev/null 2>&1 &
    say "deadman armed for ${DEADMAN_SECONDS}s (flag $DEADMAN_FLAG)"
}
disarm_deadman() { rm -f "$DEADMAN_FLAG"; }

DEV_PID=""
cleanup() {
    [ -n "$DEV_PID" ] && kill -TERM "$DEV_PID" 2>/dev/null
    for _ in $(seq 1 10); do kill -0 "$DEV_PID" 2>/dev/null || break; sleep 1; done
    kill -KILL "$DEV_PID" 2>/dev/null
    launchctl kickstart -k "$PLIST" >/dev/null 2>&1
    disarm_deadman
    if [ "$FAIL" -eq 0 ]; then say "SMOKE PASS"; else say "SMOKE FAIL: see above"; fi
    exit "$FAIL"
}
trap cleanup EXIT INT TERM

arm_deadman

# ------------------------------------------------------- put the host in shape
say "stopping the installed daemon ($PLIST) so only one Warren daemon runs"
launchctl bootout "$PLIST" >/dev/null 2>&1
sleep 2

# The guard only arms the bind on a network it has no verdict for, so a cached
# entry would make this connect take the escape and prove nothing. Forgetting
# the whole file is the honest reset: the daemon rewrites what it measures.
CACHE_BACKUP=""
if [ -f "$CACHE" ]; then
    CACHE_BACKUP="$(mktemp)"
    cp "$CACHE" "$CACHE_BACKUP"
    : > "$CACHE"
    say "verdict cache cleared (restored at exit) so the bind is armed"
fi
restore_cache() { [ -n "$CACHE_BACKUP" ] && cp "$CACHE_BACKUP" "$CACHE" && rm -f "$CACHE_BACKUP"; }

MARK="$(wc -l < "$LOG" 2>/dev/null || echo 0)"
WINDOW_START="$(date -u +%Y-%m-%d\ %H:%M:%S)"

say "starting the dev daemon"
/usr/bin/env WARREN_USE_PLAINTEXT_STORAGE=1 "$DAEMON" -vv >/dev/null 2>&1 &
DEV_PID=$!
for _ in $(seq 1 20); do [ -S "$WARREN_RPC_SOCKET_PATH" ] && break; sleep 1; done
[ -S "$WARREN_RPC_SOCKET_PATH" ] || { say "FAIL: the dev daemon never opened $WARREN_RPC_SOCKET_PATH"; FAIL=1; restore_cache; exit 1; }

# ------------------------------------------------------------------ the connect
say "connecting"
"$CLI" connect >/dev/null 2>&1
for _ in $(seq 1 40); do
    "$CLI" status 2>/dev/null | grep -q Connected && break
    sleep 1
done

# Give the guard its window plus the reconnect it should trigger.
for _ in $(seq 1 "$MAX_RECOVER_SECS"); do
    sleep 1
    ping -c 1 -t 2 1.1.1.1 >/dev/null 2>&1 && break
done

TRAFFIC=0
ping -c 3 -t 8 1.1.1.1 >/dev/null 2>&1 && TRAFFIC=1

"$CLI" disconnect >/dev/null 2>&1
sleep 2
restore_cache

# ------------------------------------------------------------------- the verdict
NEW="$(tail -n +"$((MARK + 1))" "$LOG" 2>/dev/null)"
outcome="$(printf '%s\n' "$NEW" | grep -o 'carrier_egress_guard_background outcome=[A-Za-z]*' | head -1)"
say "guard: ${outcome:-none observed}"

if printf '%s\n' "$NEW" | grep -q 'outcome=BindBlackholed'; then
    say "this host black-holes the bind, which is the case under test"
    if ! printf '%s\n' "$NEW" | grep -q 'carrier egress guard: escalating to the state machine'; then
        say "FAIL: the guard measured the blackhole and did not end the session"
        FAIL=1
    fi
    up="$(printf '%s\n' "$NEW" | grep -n 'phase=up_consumed' | head -1 | cut -d: -f1)"
    back="$(printf '%s\n' "$NEW" | grep -n 'CachedRouteOnly' | head -1 | cut -d: -f1)"
    if [ -n "$up" ] && [ -n "$back" ]; then
        t_up="$(printf '%s\n' "$NEW" | sed -n "${up}p" | cut -c2-24)"
        t_back="$(printf '%s\n' "$NEW" | sed -n "${back}p" | cut -c2-24)"
        say "first Connected at   $t_up"
        say "reconnect (escape) at $t_back"
        say "the gap between those two IS the blackout the user sees"
    else
        say "FAIL: no reconnect onto the /32 escape followed the verdict"
        FAIL=1
    fi
elif [ -n "$outcome" ]; then
    say "SKIP: this network does not black-hole the bind, nothing to prove here"
    say "(run it on a host and network where the guard reports BindBlackholed)"
else
    say "FAIL: the guard produced no verdict at all; the connect never reached Up"
    FAIL=1
fi

# The blackout has a second victim worth measuring while we hold a repro: on
# 2026-09-08 this Mac came out of one unable to resolve the very names it had
# tried to reach during it, on a resolver every direct query answered
# correctly, and only a restart of mDNSResponder cleared it. Whether the
# blackout causes that was never observed, because the logs were read after the
# fact. Read them across the window instead.
say ""
say "resolver behaviour across the blackout:"
servfail="$(log show --start "$WINDOW_START" --predicate 'process == "mDNSResponder"' --style compact 2>/dev/null \
  | grep -c "ServFail" || true)"
say "  mDNSResponder ServFail responses during the window: ${servfail:-0}"
if [ "${servfail:-0}" -gt 0 ]; then
    say "  a name the host wanted during the blackout may now be stuck. Check with:"
    say "    python3 -c \"import socket; socket.getaddrinfo('github.com',443)\""
    say "  and if it fails while a direct query answers, the cure is a restart of the"
    say "  resolver process, not a cache flush: sudo killall mDNSResponder"
fi

if [ "$TRAFFIC" -eq 1 ]; then
    say "ok: traffic flows within ${MAX_RECOVER_SECS}s of the first Connected"
else
    say "FAIL: no traffic ${MAX_RECOVER_SECS}s after connecting"
    FAIL=1
fi
