#!/usr/bin/env bash
# Prove, on a REAL exit, that a socket opened before the connect fails at once
# instead of hanging silently for the whole tunnel lifetime.
#
# Why this exists: a socket connected before the tunnel came up keeps the
# physical interface's address as its source. After the default route moves
# onto the TUN its packets enter the tunnel, and the exit's anti-spoof gate
# drops them. Until the engine answered them (warrenguard b788ddc), a connected
# UDP socket (a browser's QUIC session) got no reply and no error at all, from
# the connect to the disconnect: the user saw "connected, no internet" on every
# page the browser already had a session with (poka's Mac, 2026-09-29).
#
# The answer is unit-tested in the engine. Only a real connect proves the whole
# chain: routes, pf, the TUN, the pump and the host stack turning the ICMP
# answer into an error on the right socket.
#
#   sudo scripts/dev/macos-stale-flow-smoke.sh --beta
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
Real-exit gate for the answer to a stale-source socket.

Usage: sudo $(basename "$0") <--prod|--beta|--staging>

Env overrides:
  DEADMAN_SECONDS   deadman horizon (default 120)
  MAX_ERROR_SECS    how long a stale UDP socket may wait for its error (default 2)
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
CLI="$REPO_ROOT/target/release/warren"
[ -x "$CLI" ] || CLI="/usr/local/bin/$(warren_env_cli_name "$WARREN_PRODUCT_ENV")"
PLIST="system/com.warrenbrowse.vpn$( [ "$WARREN_PRODUCT_ENV" = prod ] && echo "" || echo ".$WARREN_PRODUCT_ENV" ).daemon"
DEADMAN_SECONDS="${DEADMAN_SECONDS:-120}"
MAX_ERROR_SECS="${MAX_ERROR_SECS:-2}"
export WARREN_RPC_SOCKET_PATH="/var/run/$PRODUCT_DIR"

say() { printf '%s\n' "$*"; }
FAIL=0

[ -x "$DAEMON" ] || { say "error: build it first: WARREN_PRODUCT_ENV=$WARREN_PRODUCT_ENV cargo build --release -p mullvad-daemon -p mullvad-cli"; exit 1; }

# ---------------------------------------------------------------- the deadman
# Detached and root-owned, so it survives this script being SIGKILLed, and it
# undoes the two things that can leave the host dark on their own: a pf ruleset
# that fails closed, and a default route pointing at a TUN nobody owns.
DEADMAN_FLAG="$(mktemp /tmp/warren-stale-flow-smoke-armed.XXXXXX)"
arm_deadman() {
    nohup bash -c '
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
        for tun in $(ifconfig -l | tr " " "\n" | grep "^utun"); do
            route -n delete -inet default -interface "$tun" >/dev/null 2>&1
            route -n delete -inet 0.0.0.0/1 -interface "$tun" >/dev/null 2>&1
            route -n delete -inet 128.0.0.0/1 -interface "$tun" >/dev/null 2>&1
        done
        for svc in $(networksetup -listallnetworkservices | tail -n +2); do
            networksetup -setdnsservers "$svc" Empty 2>/dev/null
        done
        dscacheutil -flushcache 2>/dev/null
        killall -HUP mDNSResponder 2>/dev/null
        launchctl bootstrap system "/Library/LaunchDaemons/${plist#system/}.plist" 2>/dev/null
        launchctl kickstart -k "$plist" 2>/dev/null
        rm -f "$flag"
    ' _ "$DEADMAN_FLAG" "$DEADMAN_SECONDS" "$PLIST" >/dev/null 2>&1 &
    disown
    say "deadman armed for ${DEADMAN_SECONDS}s (flag $DEADMAN_FLAG)"
}
disarm_deadman() { rm -f "$DEADMAN_FLAG"; }

DEV_PID=""
PROBE_DIR="$(mktemp -d /tmp/warren-stale-flow-smoke.XXXXXX)"
cleanup() {
    "$CLI" disconnect >/dev/null 2>&1
    [ -n "$DEV_PID" ] && kill -TERM "$DEV_PID" 2>/dev/null
    for _ in $(seq 1 10); do kill -0 "$DEV_PID" 2>/dev/null || break; sleep 1; done
    kill -KILL "$DEV_PID" 2>/dev/null
    launchctl bootstrap system "/Library/LaunchDaemons/${PLIST#system/}.plist" >/dev/null 2>&1
    launchctl kickstart -k "$PLIST" >/dev/null 2>&1
    disarm_deadman
    rm -rf "$PROBE_DIR"
    if [ "$FAIL" -eq 0 ]; then say "SMOKE PASS"; else say "SMOKE FAIL: see above"; fi
    exit "$FAIL"
}
trap cleanup EXIT INT TERM

arm_deadman

# ------------------------------------------------------- put the host in shape
say "stopping the installed daemon ($PLIST) so only one Warren daemon runs"
launchctl bootout "$PLIST" >/dev/null 2>&1
sleep 2

say "starting the dev daemon"
/usr/bin/env WARREN_USE_PLAINTEXT_STORAGE=1 "$DAEMON" -vv >/dev/null 2>&1 &
DEV_PID=$!
for _ in $(seq 1 20); do [ -S "$WARREN_RPC_SOCKET_PATH" ] && break; sleep 1; done
[ -S "$WARREN_RPC_SOCKET_PATH" ] || { say "FAIL: the dev daemon never opened $WARREN_RPC_SOCKET_PATH"; FAIL=1; exit 1; }

# ------------------------------------------------ sockets opened BEFORE connect
# A connected UDP socket (a QUIC session) and an idle TCP connection, both
# holding the physical interface's address. The probe waits for a "go" file,
# then sends on both and reports how each one ends.
cat > "$PROBE_DIR/probe.py" <<'EOF'
import os, socket, sys, time
d = sys.argv[1]
udp = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
udp.connect(("1.1.1.1", 443))
tcp = socket.create_connection(("1.1.1.1", 443), timeout=5)
print("pre-connect sources: udp=%s tcp=%s" % (udp.getsockname()[0], tcp.getsockname()[0]), flush=True)
open(os.path.join(d, "ready"), "w").close()
while not os.path.exists(os.path.join(d, "go")):
    time.sleep(0.1)
def outcome(fn):
    t0 = time.monotonic()
    try:
        fn()
        return "no error", time.monotonic() - t0
    except socket.timeout:
        return "silent timeout", time.monotonic() - t0
    except OSError as e:
        return "error %s" % os.strerror(e.errno), time.monotonic() - t0
udp.settimeout(5)
def udp_round():
    udp.send(b"\x00" * 1200)
    udp.recv(2048)
r, dt = outcome(udp_round)
print("UDP %s after %.2fs" % (r, dt), flush=True)
tcp.settimeout(5)
def tcp_round():
    tcp.send(b"\x16\x03\x01\x00\x05hello")
    if not tcp.recv(1):
        raise ConnectionResetError(54, "closed")
r, dt = outcome(tcp_round)
print("TCP %s after %.2fs" % (r, dt), flush=True)
EOF
/usr/bin/python3 "$PROBE_DIR/probe.py" "$PROBE_DIR" > "$PROBE_DIR/out" 2>&1 &
PROBE_PID=$!
for _ in $(seq 1 50); do [ -f "$PROBE_DIR/ready" ] && break; sleep 0.1; done
[ -f "$PROBE_DIR/ready" ] || { say "FAIL: the probe could not open its sockets before the connect"; cat "$PROBE_DIR/out"; FAIL=1; exit 1; }

# ------------------------------------------------------------------ the connect
say "connecting"
"$CLI" connect >/dev/null 2>&1
for _ in $(seq 1 40); do
    "$CLI" status 2>/dev/null | grep -q Connected && break
    sleep 1
done
"$CLI" status 2>/dev/null | grep -q Connected || { say "FAIL: never reached Connected"; FAIL=1; exit 1; }
sleep 2

touch "$PROBE_DIR/go"
wait "$PROBE_PID"
cat "$PROBE_DIR/out"

FRESH="$(curl -s -m6 -o /dev/null -w '%{http_code}' https://1.1.1.1/cdn-cgi/trace || echo FAIL)"
say "fresh connection through the tunnel: HTTP $FRESH"

"$CLI" disconnect >/dev/null 2>&1
sleep 2

# ------------------------------------------------------------------- the verdict
udp_line="$(grep '^UDP ' "$PROBE_DIR/out")"
case "$udp_line" in
    "UDP error "*)
        secs="$(printf '%s\n' "$udp_line" | awk '{print $(NF)}' | tr -d s)"
        if awk -v s="$secs" -v m="$MAX_ERROR_SECS" 'BEGIN { exit !(s <= m) }'; then
            say "ok: the stale UDP socket failed at once, its application reconnects"
        else
            say "FAIL: the stale UDP socket failed, but only after ${secs}s"
            FAIL=1
        fi
        ;;
    *)
        say "FAIL: the stale UDP socket got no error ($udp_line): it hangs for the tunnel lifetime"
        FAIL=1
        ;;
esac
grep -q '^TCP error ' "$PROBE_DIR/out" \
    && say "ok: the stale TCP connection failed as soon as it sent" \
    || { say "FAIL: the stale TCP connection did not fail when it sent"; FAIL=1; }
[ "$FRESH" = "200" ] || { say "FAIL: a fresh connection did not get through the tunnel"; FAIL=1; }
