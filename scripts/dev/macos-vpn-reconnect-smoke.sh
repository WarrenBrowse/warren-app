#!/usr/bin/env bash
# Reconnect-cycle datapath smoke for the macOS desktop app: prove that user
# traffic flows after a COLD connect AND after immediate reconnects. The
# 2026-07-15 incident shape was invisible to a single-connect smoke: the
# first connect worked (carrier egress guard reverted correctly) while every
# reconnect false-confirmed the IP_BOUND_IF bind and black-holed the uplink,
# so the tunnel sat "Connected" with zero egress and NAT-PMP timing out.
#
# Prerequisites: a running warren-daemon of the target environment (any
# account with an active subscription configured) and its CLI installed;
# override with WARREN_CLI / WARREN_RPC_SOCKET_PATH for an isolated dev
# daemon. Safe by construction: a trap plus an independent watchdog guarantee
# disconnect even if the tunnel kills this host's egress (the watchdog is
# local and needs no network).
#
# Usage: scripts/dev/macos-vpn-reconnect-smoke.sh <--prod|--beta|--staging>
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=../utils/product-env.sh
source "$REPO_ROOT/scripts/utils/product-env.sh"

usage() {
    cat <<EOF
Reconnect-cycle datapath smoke for the macOS desktop app.

Usage: $(basename "$0") <--prod|--beta|--staging>

Environment (required, no default: each one runs its own daemon on its own
socket, and a smoke aimed at the wrong one proves nothing about the build
under test):
  --prod       Drive the prod daemon    (/usr/local/bin/warren)
  --beta       Drive the beta daemon    (/usr/local/bin/warren-beta)
  --staging    Drive the staging daemon (/usr/local/bin/warren-staging)
  The WARREN_PRODUCT_ENV env var is used when no flag is given.

Env overrides: WARREN_CLI (CLI to drive, e.g. a dev build),
WARREN_RPC_SOCKET_PATH (daemon socket), SMOKE_CONNECT_TIMEOUT (seconds),
SMOKE_RECONNECT_CYCLES (reconnect cycles, default 2).
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
CYCLES="${SMOKE_RECONNECT_CYCLES:-2}"
FAIL=0

say() { printf '%s\n' "$*"; }
say "Environment: $WARREN_PRODUCT_ENV (cli $W, socket $WARREN_RPC_SOCKET_PATH)"

verdict() {
  if [ "$FAIL" -eq 0 ]; then
    say "RECONNECT SMOKE PASS: egress alive on cold connect and every reconnect"
  else
    say "RECONNECT SMOKE FAIL: see above"
  fi
  exit "$FAIL"
}
trap '"$W" disconnect >/dev/null 2>&1; sleep 1; "$W" disconnect >/dev/null 2>&1; verdict' EXIT
# Independent deadman: disconnect even if this script is SIGKILLed or the
# invoking agent loses its own connectivity mid-test.
nohup bash -c "sleep 240; '$W' disconnect" >/dev/null 2>&1 &

wait_connected() {
  for _ in $(seq 1 "$TIMEOUT_CONNECT"); do
    sleep 1
    "$W" status | grep -q Connected && return 0
  done
  return 1
}

probe_egress() {
  # HTTPS to a literal IP: no DNS dependency, proves the full round trip.
  local code
  code=$(curl -sS --max-time 8 -o /dev/null -w '%{http_code}' https://1.1.1.1/ 2>/dev/null)
  [ -n "$code" ] && [ "$code" != "000" ]
}

"$W" status | grep -q Disconnected || { say "precondition: expected Disconnected"; FAIL=1; exit 1; }

say "== cycle 0: cold connect =="
t0=$(date +%s)
"$W" connect
wait_connected || { say "FAIL: not Connected within ${TIMEOUT_CONNECT}s"; FAIL=1; exit 1; }
say "connected in $(( $(date +%s) - t0 ))s"
sleep 1
if probe_egress; then say "ok: egress alive after cold connect"; else say "FAIL: egress dead after cold connect"; FAIL=1; fi

for i in $(seq 1 "$CYCLES"); do
  say "== cycle $i: disconnect + immediate reconnect =="
  "$W" disconnect >/dev/null 2>&1
  sleep 1
  t0=$(date +%s)
  "$W" connect
  wait_connected || { say "FAIL: cycle $i not Connected within ${TIMEOUT_CONNECT}s"; FAIL=1; exit 1; }
  say "reconnected in $(( $(date +%s) - t0 ))s"
  sleep 1
  if probe_egress; then
    say "ok: egress alive after reconnect $i"
  else
    say "FAIL: egress dead after reconnect $i (2026-07-15 regression shape)"
    FAIL=1
  fi
done
