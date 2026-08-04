#!/usr/bin/env bash
# End-to-end datapath smoke for the macOS desktop app: connect the REAL
# daemon through a REAL exit, prove user traffic flows (ping + HTTPS + DNS
# through the tunnel), then disconnect. Loopback tests cannot catch a
# datapath that dies at the routing/socket layer (2026-07-13
# carrier-blackhole incident: every unit test was green while the tunnel
# egressed nothing), so this gate is MANDATORY before shipping a desktop
# release that touches routing, sockets, or the transport.
#
# Prerequisites: a running warren-daemon of the target environment (any
# account with an active subscription configured) and its CLI installed. Safe
# by construction: a trap plus an independent watchdog guarantee disconnect.
#
# Usage: scripts/dev/macos-vpn-smoke.sh <--prod|--beta|--staging>
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=../utils/product-env.sh
source "$REPO_ROOT/scripts/utils/product-env.sh"

usage() {
    cat <<EOF
End-to-end datapath smoke for the macOS desktop app.

Usage: $(basename "$0") <--prod|--beta|--staging>

Environment (required, no default: each one runs its own daemon on its own
socket, and a smoke aimed at the wrong one proves nothing about the build
under test):
  --prod       Drive the prod daemon    (/usr/local/bin/warren)
  --beta       Drive the beta daemon    (/usr/local/bin/warren-beta)
  --staging    Drive the staging daemon (/usr/local/bin/warren-staging)
  The WARREN_PRODUCT_ENV env var is used when no flag is given.

Env overrides: WARREN_CLI (CLI to drive, e.g. a dev build),
WARREN_RPC_SOCKET_PATH (daemon socket), SMOKE_CONNECT_TIMEOUT (seconds).
EOF
}

ENV_FLAG=""
for arg in "$@"; do
    if warren_env_flag "$arg"; then
        ENV_FLAG="$WARREN_ENV_FLAG"
        continue
    fi
    case "$arg" in
        -h|--help) usage; exit 0 ;;
        *)
            usage >&2
            printf '\nerror: unknown argument: %s\n' "$arg" >&2
            exit 1
            ;;
    esac
done
warren_env_require "$ENV_FLAG"

W="${WARREN_CLI:-/usr/local/bin/$(warren_env_cli_name "$WARREN_PRODUCT_ENV")}"
# Pin the socket too, so a CLI supplied through WARREN_CLI drives the
# environment asked for here rather than the one it was compiled with.
export WARREN_RPC_SOCKET_PATH="${WARREN_RPC_SOCKET_PATH:-/var/run/$(warren_env_product_dir "$WARREN_PRODUCT_ENV")}"
TIMEOUT_CONNECT="${SMOKE_CONNECT_TIMEOUT:-20}"
FAIL=0

say() { printf '%s\n' "$*"; }
say "Environment: $WARREN_PRODUCT_ENV (cli $W, socket $WARREN_RPC_SOCKET_PATH)"

verdict() {
  if [ "$FAIL" -eq 0 ]; then say "SMOKE PASS: tunnel carries traffic end-to-end"; else say "SMOKE FAIL: see above"; fi
  exit "$FAIL"
}
trap '"$W" disconnect >/dev/null 2>&1; sleep 1; "$W" disconnect >/dev/null 2>&1; verdict' EXIT
# Independent watchdog: disconnect even if this script is SIGKILLed.
nohup bash -c "sleep 120; '$W' disconnect" >/dev/null 2>&1 &

"$W" status | grep -q Disconnected || { say "precondition: expected Disconnected"; FAIL=1; exit 1; }
"$W" connect
connected=""
for i in $(seq 1 "$TIMEOUT_CONNECT"); do
  sleep 1
  "$W" status | grep -q Connected && { connected=1; break; }
done
[ -n "$connected" ] || { say "FAIL: not Connected within ${TIMEOUT_CONNECT}s"; FAIL=1; exit 1; }
sleep 2

if ping -c 3 -t 8 1.1.1.1 >/dev/null 2>&1; then
  say "ok: ICMP through tunnel"
else
  say "FAIL: ping 1.1.1.1 dead through tunnel"; FAIL=1
fi
code=$(curl -sS --max-time 10 -o /dev/null -w '%{http_code}' https://1.1.1.1/ 2>/dev/null)
if [ "$code" != "000" ] && [ -n "$code" ]; then
  say "ok: HTTPS to literal IP (HTTP $code)"
else
  say "FAIL: HTTPS to 1.1.1.1 dead through tunnel"; FAIL=1
fi
code=$(curl -sS --max-time 10 -o /dev/null -w '%{http_code}' https://example.com/ 2>/dev/null)
if [ "$code" = "200" ]; then
  say "ok: DNS + HTTPS through tunnel"
else
  say "FAIL: DNS or HTTPS to example.com dead through tunnel (HTTP ${code:-none})"; FAIL=1
fi
