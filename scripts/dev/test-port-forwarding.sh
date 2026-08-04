#!/usr/bin/env bash
#
# test-port-forwarding.sh, Deep end-to-end test for an inbound port
# forward (e.g. Warren NAT-PMP) from the public internet to this host.
#
# Mechanism
# ---------
# A forwarded port only "works" if a packet sent from OUTSIDE the tunnel
# to the exit's public IP:external_port is delivered, through the tunnel,
# to a listener on THIS machine. So the test needs two things:
#
#   1. A local listener bound here on the forwarded (internal) port.
#   2. An external vantage point, a host that is NOT routed through this
#      machine's tunnel, to fire the probe. We reach it over SSH and run
#      a tiny ephemeral Python prober there (nothing is left behind).
#
# The prober sends N probes to <target-ip>:<external-port>; the local
# listener echoes each one back. We then correlate both sides:
#
#   external prober ── UDP/TCP ──▶ exit public IP:external_port
#                                        │ (DNAT, through tunnel)
#                                        ▼
#                                  this host:listen_port  (listener)
#                                        │ echo
#                                        ▼  (back through tunnel)
#   external prober ◀────────────── exit ◀──
#
# This proves: inbound delivery, the return path, source-IP preservation
# (pure DNAT vs. SNAT), payload integrity, and round-trip latency.
#
# Portability: bash 3.2+ (macOS) and bash 4+/5 (Linux). The actual I/O is
# done in Python 3 (present on both sides) so behaviour is identical
# across platforms, far more reliable than juggling BSD vs. GNU `nc`.
#
# Security: SSH runs in BatchMode (never hangs on a password prompt),
# temp files live in a 0700 mktemp dir, inputs are validated before use,
# and the remote command is built only from validated tokens.

set -euo pipefail

PROG="$(basename "$0")"

# ─────────────────────────────────────────────────────────────────────
# Defaults
# ─────────────────────────────────────────────────────────────────────
PROTO="udp"
PROTO_UC="UDP"
LISTEN_PORT=""
EXTERNAL_PORT=""          # defaults to the port granted by the exit
EXTERNAL_PORT_SET="0"     # 1 once the user pins it explicitly
TARGET_IP=""              # exit public IP; auto-detected if empty
SSH_TARGET=""             # user@host
SSH_USER=""
SSH_HOST=""
SSH_PORT=""               # empty: let ssh_config decide, do not force 22
IDENTITY=""
WARREN_CLI=""             # warren CLI used to read the live mapping state
SKIP_DAEMON_CHECK="0"
MODE="echo"               # echo: our own listener; connect: probe a live app
STRICT_HOSTKEY="accept-new"
COUNT="3"
PROBE_TIMEOUT="2"         # seconds to wait for each echo
PROBE_INTERVAL="0.3"      # seconds between probes
PAYLOAD_SIZE="32"         # bytes per probe (bump to ~1400 to exercise MTU)
DO_CAPTURE="0"
IFACE=""
VERBOSE="0"

# ─────────────────────────────────────────────────────────────────────
# Colors (only when stdout is a TTY)
# ─────────────────────────────────────────────────────────────────────
if [ -t 1 ]; then
  C_RED=$'\033[0;31m'; C_GRN=$'\033[0;32m'; C_YEL=$'\033[0;33m'
  C_BLU=$'\033[0;34m'; C_CYN=$'\033[0;36m'; C_BOLD=$'\033[1m'; C_RST=$'\033[0m'
else
  C_RED=""; C_GRN=""; C_YEL=""; C_BLU=""; C_CYN=""; C_BOLD=""; C_RST=""
fi

info() { printf "%s[info]%s  %s\n"  "$C_BLU" "$C_RST" "$*"; }
ok()   { printf "%s[ok]%s    %s\n"  "$C_GRN" "$C_RST" "$*"; }
warn() { printf "%s[warn]%s  %s\n"  "$C_YEL" "$C_RST" "$*" >&2; }
err()  { printf "%s[error]%s %s\n"  "$C_RED" "$C_RST" "$*" >&2; }
die()  { err "$@"; exit 1; }

# ─────────────────────────────────────────────────────────────────────
# Usage
# ─────────────────────────────────────────────────────────────────────
usage() {
  cat <<EOF
${C_BOLD}$PROG${C_RST}, deep end-to-end test for an inbound port forward

${C_GRN}Usage:${C_RST}
  $PROG --port <PORT> --proto <udp|tcp> --ssh <user@host> [options]

${C_GRN}Required:${C_RST}
  -p, --port PORT          Local port to listen on (the forwarded / internal port).
                           Must match a rule in \`warren port-forward get\`.
  -s, --ssh  USER@HOST[:PORT]
                           External host used as the prober. MUST be outside this
                           machine's tunnel. (Or use --ssh-user + --ssh-host.)

${C_GRN}Options:${C_RST}
  -P, --proto PROTO        udp (default) or tcp.
  -t, --target-ip IP       Exit public IP the prober connects to.
                           Default: auto-detected current egress IP.
  -e, --external-port PORT Public port on the exit to probe. Default: the port
                           the exit actually GRANTED, read from the daemon
                           (\`warren port-forward status\`); falls back to --port.
      --ssh-user USER      SSH user (alternative to user@host form).
      --ssh-host HOST      SSH host (alternative to user@host form).
      --ssh-port PORT      SSH port. Default: whatever ~/.ssh/config resolves
                           for that host, else ssh's own default (22).
  -i, --identity FILE      SSH private key file.
      --insecure-hostkey   Set StrictHostKeyChecking=no (default: accept-new).
  -c, --count N            Number of probes (default 3).
  -w, --timeout SECS       Per-probe echo timeout (default 2).
      --interval SECS      Delay between probes (default 0.3).
      --size BYTES         Probe payload size (default 32; try ~1400 for MTU).
      --capture            Also sniff the tunnel interface (needs sudo + --iface).
      --iface IFACE        Interface to capture on (e.g. utun4 / wg0-mullvad).
      --warren-cli PATH    warren CLI to query (default: PATH, then target/{debug,release}).
      --no-daemon-check    Skip the daemon preflight (mapping state + granted port).
      --no-listener        TCP only: do not bind locally, just check that the app
                           already listening on the port is reachable from outside.
                           Applied automatically when the port is already in use.
  -v, --verbose            Show per-probe detail.
  -h, --help               This help.

${C_GRN}Examples:${C_RST}
  # Test a UDP forward on 55124, probing from a server you control:
  $PROG --port 55124 --proto udp --ssh root@203.0.113.9

  # Prober whose sshd listens on a non-standard port:
  $PROG -p 55124 -s poka@p2p.legal --ssh-port 30022
  $PROG -p 55124 -s poka@p2p.legal:30022

  # TCP, explicit exit IP + capture on the macOS tunnel interface:
  $PROG -p 55124 -P tcp -s admin@srv -t 204.168.207.130 --capture --iface utun4

${C_GRN}Setting a forward up first:${C_RST}
  warren port-forward enable --internal-port 55124 --protocol udp
  warren port-forward status          # wait for "MAPPED, public port N"

${C_YEL}Exit codes:${C_RST} 0 PASS · 2 PARTIAL · 3 FAIL · 1 usage/setup error
EOF
}

# ─────────────────────────────────────────────────────────────────────
# Argument parsing (manual: portable, supports --opt val and --opt=val)
# ─────────────────────────────────────────────────────────────────────
# Splits "--opt=value" into "--opt" "value" so the loop below is uniform.
ARGS=()
for a in "$@"; do
  case "$a" in
    --*=*) ARGS+=("${a%%=*}" "${a#*=}") ;;
    *)     ARGS+=("$a") ;;
  esac
done
set -- "${ARGS[@]:-}"

while [ "$#" -gt 0 ]; do
  case "${1:-}" in
    -p|--port)            LISTEN_PORT="${2:-}"; shift 2 ;;
    -P|--proto)           PROTO="${2:-}"; shift 2 ;;
    -t|--target-ip)       TARGET_IP="${2:-}"; shift 2 ;;
    -e|--external-port)   EXTERNAL_PORT="${2:-}"; EXTERNAL_PORT_SET="1"; shift 2 ;;
    -s|--ssh)             SSH_TARGET="${2:-}"; shift 2 ;;
    --ssh-user)           SSH_USER="${2:-}"; shift 2 ;;
    --ssh-host)           SSH_HOST="${2:-}"; shift 2 ;;
    --ssh-port)           SSH_PORT="${2:-}"; shift 2 ;;
    -i|--identity)        IDENTITY="${2:-}"; shift 2 ;;
    --insecure-hostkey)   STRICT_HOSTKEY="no"; shift ;;
    -c|--count)           COUNT="${2:-}"; shift 2 ;;
    -w|--timeout)         PROBE_TIMEOUT="${2:-}"; shift 2 ;;
    --interval)           PROBE_INTERVAL="${2:-}"; shift 2 ;;
    --size)               PAYLOAD_SIZE="${2:-}"; shift 2 ;;
    --capture)            DO_CAPTURE="1"; shift ;;
    --iface)              IFACE="${2:-}"; shift 2 ;;
    --warren-cli)         WARREN_CLI="${2:-}"; shift 2 ;;
    --no-daemon-check)    SKIP_DAEMON_CHECK="1"; shift ;;
    --no-listener)        MODE="connect"; shift ;;
    -v|--verbose)         VERBOSE="1"; shift ;;
    -h|--help)            usage; exit 0 ;;
    "")                   shift ;;
    *)                    die "Unknown argument: $1 (try --help)" ;;
  esac
done

# ─────────────────────────────────────────────────────────────────────
# Validation
# ─────────────────────────────────────────────────────────────────────
require_cmd() { command -v "$1" >/dev/null 2>&1 || die "'$1' not found in PATH"; }
require_cmd python3
require_cmd ssh

is_port() { printf '%s' "$1" | grep -qE '^[0-9]+$' && [ "$1" -ge 1 ] && [ "$1" -le 65535 ]; }
is_uint() { printf '%s' "$1" | grep -qE '^[0-9]+$' && [ "$1" -ge 1 ]; }

PROTO="$(printf '%s' "$PROTO" | tr '[:upper:]' '[:lower:]')"
[ "$PROTO" = "udp" ] || [ "$PROTO" = "tcp" ] || die "--proto must be 'udp' or 'tcp' (got '$PROTO')"
PROTO_UC="$(printf '%s' "$PROTO" | tr '[:lower:]' '[:upper:]')"

[ -n "$LISTEN_PORT" ] || { usage; echo; die "--port is required"; }
is_port "$LISTEN_PORT" || die "--port must be 1..65535 (got '$LISTEN_PORT')"

if [ "$EXTERNAL_PORT_SET" = "1" ]; then
  is_port "$EXTERNAL_PORT" || die "--external-port must be 1..65535 (got '$EXTERNAL_PORT')"
fi

is_uint "$COUNT" || die "--count must be a positive integer"
printf '%s' "$PROBE_TIMEOUT"  | grep -qE '^[0-9]+(\.[0-9]+)?$' || die "--timeout must be numeric"
printf '%s' "$PROBE_INTERVAL" | grep -qE '^[0-9]+(\.[0-9]+)?$' || die "--interval must be numeric"
is_uint "$PAYLOAD_SIZE" || die "--size must be a positive integer"
[ "$PAYLOAD_SIZE" -ge 16 ] || die "--size must be >= 16 (header needs room)"

# Resolve SSH target into user@host, with an optional :port suffix. The
# IPv6-literal form [::1]:22 is not handled: ssh_config is the way in.
if [ -n "$SSH_TARGET" ]; then
  case "$SSH_TARGET" in
    *@*) SSH_USER="${SSH_TARGET%@*}"; SSH_HOST="${SSH_TARGET#*@}" ;;
    *)   SSH_HOST="$SSH_TARGET" ;;
  esac
  case "$SSH_HOST" in
    *:[0-9]*) SSH_PORT="${SSH_HOST##*:}"; SSH_HOST="${SSH_HOST%:*}" ;;
  esac
fi
[ -n "$SSH_HOST" ] || { usage; echo; die "--ssh user@host (or --ssh-host) is required"; }
# User and port are both optional: a bare host may be an ssh_config Host
# alias that already carries User/Port/IdentityFile. Forcing -p 22 or a
# guessed user would override that entry and break the connection.
[ -z "$SSH_PORT" ] || is_port "$SSH_PORT" || die "--ssh-port must be 1..65535"
if [ -n "$SSH_USER" ]; then
  SSH_DEST="${SSH_USER}@${SSH_HOST}"
else
  SSH_DEST="$SSH_HOST"
fi

if [ -n "$IDENTITY" ] && [ ! -r "$IDENTITY" ]; then
  die "--identity file not readable: $IDENTITY"
fi

if [ -n "$WARREN_CLI" ] && [ ! -x "$WARREN_CLI" ]; then
  die "--warren-cli not executable: $WARREN_CLI"
fi

# A UDP forward cannot be judged without an echoing listener of ours: a
# datagram sent into a live app produces no observable answer, so silence
# would be indistinguishable from a dead forward.
if [ "$MODE" = "connect" ] && [ "$PROTO" = "udp" ]; then
  die "--no-listener needs --proto tcp; a UDP forward needs our own listener to echo."
fi

if [ "$DO_CAPTURE" = "1" ] && [ -z "$IFACE" ]; then
  die "--capture requires --iface (e.g. utun4 on macOS, wg0-mullvad on Linux)"
fi

if [ "$LISTEN_PORT" -lt 1024 ] && [ "$(id -u)" -ne 0 ]; then
  warn "Listening on privileged port $LISTEN_PORT may require root, bind may fail."
fi

# ─────────────────────────────────────────────────────────────────────
# Temp workspace + cleanup trap (idempotent)
# ─────────────────────────────────────────────────────────────────────
umask 077
WORKDIR="$(mktemp -d "${TMPDIR:-/tmp}/warren-pf.XXXXXX")"
READY_FILE="$WORKDIR/ready"
RESULT_FILE="$WORKDIR/received.log"
LISTENER_LOG="$WORKDIR/listener.log"
PROBER_OUT="$WORKDIR/prober.out"
CAPTURE_PCAP="$WORKDIR/capture.pcap"
CAPTURE_LOG="$WORKDIR/capture.log"
: > "$RESULT_FILE"

LISTENER_PID=""
CAPTURE_PID=""
CLEANED="0"

cleanup() {
  [ "$CLEANED" = "1" ] && return 0
  CLEANED="1"
  if [ -n "$LISTENER_PID" ] && kill -0 "$LISTENER_PID" 2>/dev/null; then
    kill "$LISTENER_PID" 2>/dev/null || true
    wait "$LISTENER_PID" 2>/dev/null || true
  fi
  if [ -n "$CAPTURE_PID" ] && kill -0 "$CAPTURE_PID" 2>/dev/null; then
    sudo -n kill "$CAPTURE_PID" 2>/dev/null || kill "$CAPTURE_PID" 2>/dev/null || true
    wait "$CAPTURE_PID" 2>/dev/null || true
  fi
  rm -rf "$WORKDIR" 2>/dev/null || true
}
trap cleanup EXIT
trap 'err "Interrupted"; exit 130' INT TERM HUP

# Run identifier so probes from this run can be told apart from any
# stray traffic or a previous run's leftovers.
RUN_ID="$(head -c4 /dev/urandom 2>/dev/null | od -An -tx1 2>/dev/null | tr -d ' \n')"
[ -n "$RUN_ID" ] || RUN_ID="$$$(date +%s)"

# Listener must outlive the whole probe sequence (+ SSH latency margin).
LISTENER_MAX_SECS="$(python3 -c "print(int(max(45, $COUNT*($PROBE_TIMEOUT+$PROBE_INTERVAL)+25)))")"

# ─────────────────────────────────────────────────────────────────────
# Embedded Python: local listener
#   argv: proto port run_id expect ready_file result_file max_secs
#   Writes one line per received probe to result_file:
#     <idx|stray>|<src_ip>|<src_port>|<len>|<recv_epoch>
# ─────────────────────────────────────────────────────────────────────
cat > "$WORKDIR/listener.py" <<'PY'
import os, socket, sys, time, signal

proto, port, run_id, expect, ready_file, result_file, max_secs = sys.argv[1:8]
port = int(port); expect = int(expect); max_secs = float(max_secs)
prefix = ("WPF:" + run_id + ":").encode()

signal.signal(signal.SIGTERM, lambda *_: sys.exit(0))

res = open(result_file, "a", buffering=1)

def classify(data):
    # Returns the probe index if this datagram belongs to our run,
    # else None (stray / unrelated traffic).
    if not data.startswith(prefix):
        return None
    try:
        return int(data[len(prefix):].split(b":", 1)[0])
    except Exception:
        return None

def record(idx, addr, n):
    tag = "stray" if idx is None else str(idx)
    res.write("%s|%s|%s|%d|%.6f\n" % (tag, addr[0], addr[1], n, time.time()))

deadline = time.time() + max_secs
got = set()

try:
    if proto == "udp":
        s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        s.bind(("0.0.0.0", port))
        open(ready_file, "w").close()
        s.settimeout(0.5)
        while time.time() < deadline and len(got) < expect:
            try:
                data, addr = s.recvfrom(65535)
            except socket.timeout:
                continue
            idx = classify(data)
            record(idx, addr, len(data))
            try:
                s.sendto(data, addr)   # echo identical payload back
            except OSError:
                pass
            if idx is not None:
                got.add(idx)
    else:  # tcp
        s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        s.bind(("0.0.0.0", port))
        s.listen(16)
        open(ready_file, "w").close()
        s.settimeout(0.5)
        while time.time() < deadline and len(got) < expect:
            try:
                conn, addr = s.accept()
            except socket.timeout:
                continue
            with conn:
                conn.settimeout(2.0)
                buf = b""
                try:
                    while True:
                        chunk = conn.recv(65535)
                        if not chunk:
                            break          # peer half-closed: message complete
                        buf += chunk
                        if len(buf) > 1 << 20:
                            break
                except (socket.timeout, OSError):
                    pass
                idx = classify(buf)
                record(idx, addr, len(buf))
                try:
                    conn.sendall(buf)       # echo back
                except OSError:
                    pass
            if idx is not None:
                got.add(idx)
except OSError as e:
    sys.stderr.write("listener bind/run error: %s\n" % e)
    sys.exit(1)
finally:
    res.close()
PY

# ─────────────────────────────────────────────────────────────────────
# Embedded Python: remote prober (shipped over SSH stdin, no temp file)
#   argv: proto target_ip target_port count timeout interval size run_id
#   Prints per-probe lines + a final machine-parseable SUMMARY line.
# ─────────────────────────────────────────────────────────────────────
cat > "$WORKDIR/prober.py" <<'PY'
import socket, sys, time

proto, ip, port, count, timeout, interval, size, run_id, mode = sys.argv[1:10]
port = int(port); count = int(count)
timeout = float(timeout); interval = float(interval); size = int(size)

stype = socket.SOCK_DGRAM if proto == "udp" else socket.SOCK_STREAM
try:
    info = socket.getaddrinfo(ip, port, socket.AF_UNSPEC, stype)[0]
except OSError as e:
    print("SUMMARY sent=0 echo_ok=0 payload_ok=0 src=- rtt_min=- rtt_avg=- rtt_max=- err=resolve:%s" % e)
    sys.exit(0)
af, _, _, _, sa = info

sent = echo_ok = payload_ok = 0
src = "-"
rtts = []

for i in range(count):
    head = ("WPF:%s:%d:" % (run_id, i)).encode()
    payload = head + (b"." * max(0, size - len(head)))
    s = socket.socket(af, stype)
    s.settimeout(timeout)
    t0 = time.time()
    try:
        s.connect(sa)
        try:
            src = s.getsockname()[0]
        except OSError:
            pass
        if mode == "connect":
            # The live app owns the port, so a completed TCP handshake is
            # the whole proof: the SYN crossed the exit DNAT and the
            # tunnel, and the app answered. Send nothing into it.
            rtt = (time.time() - t0) * 1000.0
            sent += 1; echo_ok += 1; payload_ok += 1
            rtts.append(rtt)
            print("PROBE %d connect-ok rtt=%.1fms" % (i, rtt))
            if i + 1 < count:
                time.sleep(interval)
            continue
        s.sendall(payload)
        sent += 1
        if proto == "tcp":
            try:
                s.shutdown(socket.SHUT_WR)   # signal end-of-message
            except OSError:
                pass
        buf = b""
        try:
            while True:
                chunk = s.recv(65535)
                if not chunk:
                    break
                buf += chunk
                if proto == "udp" or len(buf) >= len(payload):
                    break
        except socket.timeout:
            pass
        if buf:
            echo_ok += 1
            rtts.append((time.time() - t0) * 1000.0)
            if buf == payload:
                payload_ok += 1
            print("PROBE %d ok echo=%dB rtt=%.1fms" % (i, len(buf), (time.time()-t0)*1000.0))
        else:
            print("PROBE %d sent-no-echo" % i)
    except Exception as e:
        print("PROBE %d error %s" % (i, e))
    finally:
        s.close()
    if i + 1 < count:
        time.sleep(interval)

if rtts:
    rmin, rmax = min(rtts), max(rtts)
    ravg = sum(rtts) / len(rtts)
    print("SUMMARY sent=%d echo_ok=%d payload_ok=%d src=%s rtt_min=%.1f rtt_avg=%.1f rtt_max=%.1f"
          % (sent, echo_ok, payload_ok, src, rmin, ravg, rmax))
else:
    print("SUMMARY sent=%d echo_ok=%d payload_ok=%d src=%s rtt_min=- rtt_avg=- rtt_max=-"
          % (sent, echo_ok, payload_ok, src))
PY

# ─────────────────────────────────────────────────────────────────────
# Daemon preflight: live mapping state + the port the exit GRANTED
# ─────────────────────────────────────────────────────────────────────
# The exit is free to grant a public port other than the suggested one,
# and a rule can sit in requesting/rate-limited/failed. Probing then
# proves nothing: the packets have nowhere to land. The daemon knows
# both facts, so read them before spending an SSH round trip.
find_warren_cli() {
  local repo_root cand
  if [ -n "$WARREN_CLI" ]; then
    printf '%s' "$WARREN_CLI"; return 0
  fi
  if command -v warren >/dev/null 2>&1; then
    command -v warren; return 0
  fi
  repo_root="$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)"
  for cand in "$repo_root/target/debug/warren" "$repo_root/target/release/warren"; do
    [ -x "$cand" ] && { printf '%s' "$cand"; return 0; }
  done
  return 1
}

# Prints "<state>|<public_port>" for our (internal port, protocol) rule.
# state: mapped | pending | absent, public_port empty unless mapped.
daemon_mapping() {
  local cli line
  cli="$1"
  line="$("$cli" port-forward status 2>/dev/null \
          | grep -E "^${LISTEN_PORT}/${PROTO_UC}: " | tail -1)" || true
  [ -n "$line" ] || { printf 'absent|'; return 0; }
  case "$line" in
    *": MAPPED"*)
      printf 'mapped|%s' "$(printf '%s' "$line" | grep -oE 'public port [0-9]+' | grep -oE '[0-9]+' || true)" ;;
    *) printf 'pending|' ;;
  esac
}

GRANTED_PORT=""
if [ "$SKIP_DAEMON_CHECK" = "1" ]; then
  info "Daemon preflight skipped (--no-daemon-check)."
elif WARREN_BIN="$(find_warren_cli)"; then
  info "Reading live mapping state from $WARREN_BIN …"
  MAP="$(daemon_mapping "$WARREN_BIN")"
  MAP_STATE="${MAP%%|*}"; GRANTED_PORT="${MAP#*|}"
  case "$MAP_STATE" in
    mapped)
      if [ -n "$GRANTED_PORT" ]; then
        ok "Mapping live: ${LISTEN_PORT}/${PROTO_UC} granted public port $GRANTED_PORT"
      else
        ok "Mapping live: ${LISTEN_PORT}/${PROTO_UC} (public port not reported)"
      fi ;;
    pending)
      "$WARREN_BIN" port-forward status 2>&1 | sed 's/^/    /' >&2 || true
      die "Rule ${LISTEN_PORT}/${PROTO_UC} exists but is not MAPPED (see above); nothing to probe yet." ;;
    *)
      "$WARREN_BIN" port-forward get 2>&1 | sed 's/^/    /' >&2 || true
      die "No ${LISTEN_PORT}/${PROTO_UC} rule (see settings above). Add one first:
    warren port-forward enable --internal-port $LISTEN_PORT --protocol $PROTO" ;;
  esac
else
  warn "warren CLI not found, skipping the mapping preflight (use --warren-cli)."
fi

if [ "$EXTERNAL_PORT_SET" = "0" ]; then
  # The granted port is the only one the exit actually DNATs; falling
  # back to the internal port is a guess that silently tests nothing.
  if [ -n "$GRANTED_PORT" ]; then
    EXTERNAL_PORT="$GRANTED_PORT"
  else
    EXTERNAL_PORT="$LISTEN_PORT"
    warn "Granted public port unknown, assuming $EXTERNAL_PORT (override with --external-port)."
  fi
fi
is_port "$EXTERNAL_PORT" || die "external port must be 1..65535 (got '$EXTERNAL_PORT')"

# ─────────────────────────────────────────────────────────────────────
# Auto-detect the exit/egress IP if not provided
# ─────────────────────────────────────────────────────────────────────
# Reports every failed attempt: a bare "could not detect" hides whether
# the tunnel is down, DNS is broken, or the echo services are just out.
detect_egress_ip() {
  command -v curl >/dev/null 2>&1 || { warn "curl not found in PATH"; return 1; }
  local url body ip rc
  # The last entry is an IP literal, so a resolver broken inside the
  # tunnel cannot masquerade as a dead data path.
  for url in https://api.ipify.org https://ifconfig.me/ip https://icanhazip.com \
             https://1.1.1.1/cdn-cgi/trace; do
    rc=0
    body="$(curl -4fsS --max-time 8 "$url" 2>/dev/null)" || rc=$?
    if [ "$rc" -ne 0 ]; then
      warn "  $url: curl failed (exit $rc)"
      continue
    fi
    # cdn-cgi/trace is key=value lines; the plain services echo the IP.
    ip="$(printf '%s\n' "$body" | sed -n 's/^ip=//p' | head -1)"
    [ -n "$ip" ] || ip="$(printf '%s' "$body" | tr -d '[:space:]')"
    if printf '%s' "$ip" | grep -qE '^[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+$'; then
      printf '%s' "$ip"; return 0
    fi
    warn "  $url: unexpected body, no IPv4 found"
  done
  return 1
}

if [ -z "$TARGET_IP" ]; then
  info "Auto-detecting exit/egress IP…"
  if TARGET_IP="$(detect_egress_ip)"; then
    ok "Egress IP: $TARGET_IP  (the prober will target this)"
  else
    err "Could not auto-detect the egress IP. Either the tunnel is not"
    err "carrying traffic right now, or the echo services are unreachable."
    die "Check \`warren status\`, then pass --target-ip <exit public IP>."
  fi
fi

# ─────────────────────────────────────────────────────────────────────
# SSH option array (built once, reused)
# ─────────────────────────────────────────────────────────────────────
SSH_OPTS=(-o BatchMode=yes -o ConnectTimeout=10 -o "StrictHostKeyChecking=$STRICT_HOSTKEY"
          -o ServerAliveInterval=5 -o ServerAliveCountMax=3)
[ -n "$SSH_PORT" ] && SSH_OPTS+=(-p "$SSH_PORT")
[ -n "$IDENTITY" ] && SSH_OPTS+=(-i "$IDENTITY")

# ─────────────────────────────────────────────────────────────────────
# Pre-flight: confirm the prober host is reachable and not behind us
# ─────────────────────────────────────────────────────────────────────
info "Checking SSH access to $SSH_DEST …"
REMOTE_PREFLIGHT="$(ssh "${SSH_OPTS[@]}" "$SSH_DEST" \
  'command -v python3 >/dev/null 2>&1 && echo PY_OK || echo PY_MISSING' 2>&1)" || \
  die "SSH to $SSH_DEST failed: $REMOTE_PREFLIGHT"
case "$REMOTE_PREFLIGHT" in
  *PY_OK*) ok "Reachable; python3 present on $SSH_HOST" ;;
  *)       die "python3 not found on $SSH_HOST, required for the prober" ;;
esac

if [ "$SSH_HOST" = "$TARGET_IP" ]; then
  warn "Prober host == target IP. Locally-generated packets bypass the"
  warn "exit's PREROUTING DNAT, so this cannot validate the forward."
  warn "Use a DIFFERENT external host as the prober."
fi

# ─────────────────────────────────────────────────────────────────────
# Start the local listener; wait until bound (or detect early failure)
# ─────────────────────────────────────────────────────────────────────
port_holder() {
  command -v lsof >/dev/null 2>&1 || return 0
  lsof -nP -i"${PROTO_UC}:${LISTEN_PORT}" 2>/dev/null \
    | awk 'NR==2 {printf "%s (pid %s)", $1, $2}'
}

if [ "$MODE" = "echo" ]; then
  info "Starting local $PROTO_UC listener on 0.0.0.0:$LISTEN_PORT …"
  python3 "$WORKDIR/listener.py" "$PROTO" "$LISTEN_PORT" "$RUN_ID" "$COUNT" \
          "$READY_FILE" "$RESULT_FILE" "$LISTENER_MAX_SECS" \
          > "$LISTENER_LOG" 2>&1 &
  LISTENER_PID=$!

  waited=0
  while [ ! -f "$READY_FILE" ]; do
    if ! kill -0 "$LISTENER_PID" 2>/dev/null; then
      LISTENER_PID=""
      if grep -q 'Address already in use' "$LISTENER_LOG" 2>/dev/null; then
        HOLDER="$(port_holder)"
        # Testing the forward of an app that is already running is the
        # normal case, so an occupied TCP port switches the test to the
        # live app rather than aborting.
        if [ "$PROTO" = "tcp" ]; then
          warn "Port $LISTEN_PORT already bound by ${HOLDER:-another process}."
          warn "Probing that live listener instead (connect-only)."
          MODE="connect"
          break
        fi
        err "UDP port $LISTEN_PORT is already bound by ${HOLDER:-another process}."
        die "Stop it and retry: a UDP forward needs our own echoing listener."
      fi
      err "Listener failed to start:"
      sed 's/^/    /' "$LISTENER_LOG" >&2 || true
      exit 1
    fi
    sleep 0.2
    waited=$((waited + 1))
    [ "$waited" -gt 50 ] && die "Listener did not bind within 10s"
  done
  [ "$MODE" = "echo" ] && ok "Listener bound (pid $LISTENER_PID), echoing back each probe."
else
  HOLDER="$(port_holder)"
  info "No local listener: probing ${HOLDER:-whatever already serves $LISTEN_PORT}."
fi

# ─────────────────────────────────────────────────────────────────────
# Optional packet capture on the tunnel interface (best-effort)
# ─────────────────────────────────────────────────────────────────────
if [ "$DO_CAPTURE" = "1" ]; then
  if command -v tcpdump >/dev/null 2>&1 && sudo -n true 2>/dev/null; then
    info "Capturing on $IFACE ($PROTO port $LISTEN_PORT) …"
    sudo nohup tcpdump -n -i "$IFACE" -w "$CAPTURE_PCAP" \
         "$PROTO port $LISTEN_PORT" > "$CAPTURE_LOG" 2>&1 &
    CAPTURE_PID=$!
    sleep 1
  else
    warn "Capture skipped: tcpdump missing or passwordless sudo unavailable."
    DO_CAPTURE="0"
  fi
fi

# ─────────────────────────────────────────────────────────────────────
# Fire the probes from the external host
# ─────────────────────────────────────────────────────────────────────
if [ "$MODE" = "connect" ]; then
  info "Firing $COUNT $PROTO_UC connect(s): $SSH_HOST ─▶ $TARGET_IP:$EXTERNAL_PORT"
else
  info "Firing $COUNT $PROTO_UC probe(s): $SSH_HOST ─▶ $TARGET_IP:$EXTERNAL_PORT (payload ${PAYLOAD_SIZE}B)"
fi
if [ "$VERBOSE" = "1" ]; then
  info "remote: python3 - $PROTO $TARGET_IP $EXTERNAL_PORT $COUNT $PROBE_TIMEOUT $PROBE_INTERVAL $PAYLOAD_SIZE $RUN_ID"
fi

# Remote args are all validated tokens (proto/ip/ints/hex), no shell
# metacharacters, so passing them on the ssh command line is safe.
ssh "${SSH_OPTS[@]}" "$SSH_DEST" \
    python3 - "$PROTO" "$TARGET_IP" "$EXTERNAL_PORT" "$COUNT" \
              "$PROBE_TIMEOUT" "$PROBE_INTERVAL" "$PAYLOAD_SIZE" "$RUN_ID" "$MODE" \
    < "$WORKDIR/prober.py" > "$PROBER_OUT" 2>&1 || \
    warn "Prober returned non-zero (see detail below)"

# Grace period so any last in-flight echo / late datagram is recorded.
sleep 1

# Stop the listener now that probing is done.
if [ -n "$LISTENER_PID" ] && kill -0 "$LISTENER_PID" 2>/dev/null; then
  kill "$LISTENER_PID" 2>/dev/null || true
  wait "$LISTENER_PID" 2>/dev/null || true
fi
if [ "$DO_CAPTURE" = "1" ] && [ -n "$CAPTURE_PID" ]; then
  sudo -n kill "$CAPTURE_PID" 2>/dev/null || true
  wait "$CAPTURE_PID" 2>/dev/null || true
fi

# ─────────────────────────────────────────────────────────────────────
# Correlate both sides
# ─────────────────────────────────────────────────────────────────────
# Local listener truth: distinct matched probe indices = inbound delivered.
INBOUND="$(awk -F'|' '$1!="stray"{print $1}' "$RESULT_FILE" 2>/dev/null | sort -u | wc -l | tr -d ' ')"
STRAY="$(awk -F'|'  '$1=="stray"' "$RESULT_FILE" 2>/dev/null | wc -l | tr -d ' ')"
SRC_IPS="$(awk -F'|' '$1!="stray"{print $2}' "$RESULT_FILE" 2>/dev/null | sort -u | paste -sd, - 2>/dev/null)"
[ -z "$INBOUND" ] && INBOUND=0
[ -z "$STRAY" ]   && STRAY=0

# Prober truth (from SUMMARY line).
get_kv() { grep -oE "$1=[^ ]+" "$PROBER_OUT" 2>/dev/null | tail -1 | cut -d= -f2 || true; }
P_SENT="$(get_kv sent)";       : "${P_SENT:=0}"
P_ECHO="$(get_kv echo_ok)";    : "${P_ECHO:=0}"
P_PLOK="$(get_kv payload_ok)"; : "${P_PLOK:=0}"
P_SRC="$(get_kv src)";         : "${P_SRC:=-}"
R_MIN="$(get_kv rtt_min)";     : "${R_MIN:=-}"
R_AVG="$(get_kv rtt_avg)";     : "${R_AVG:=-}"
R_MAX="$(get_kv rtt_max)";     : "${R_MAX:=-}"

# Source-IP preservation. The prober only knows its LOCAL socket address,
# which is a private one whenever the prober sits behind its own NAT, so
# comparing it to what the listener saw would flag a healthy pure-DNAT
# exit as SNAT. The exit-side question is instead: is the observed source
# the exit's own address (masquerade) or a tunnel-internal one?
is_private_ip() {
  case "$1" in
    10.*|127.*|169.254.*|192.168.*) return 0 ;;
    172.1[6-9].*|172.2[0-9].*|172.3[01].*) return 0 ;;
    100.6[4-9].*|100.[7-9][0-9].*|100.1[01][0-9].*|100.12[0-7].*) return 0 ;;
    *) return 1 ;;
  esac
}

SRC_MATCH="unknown"
if [ -n "$SRC_IPS" ]; then
  SRC_MATCH="preserved"
  case ",$SRC_IPS," in
    *,"$TARGET_IP",*) SRC_MATCH="snat" ;;
  esac
  if [ "$SRC_MATCH" = "preserved" ]; then
    for observed in $(printf '%s' "$SRC_IPS" | tr ',' ' '); do
      is_private_ip "$observed" && SRC_MATCH="private"
    done
  fi
  if [ "$SRC_MATCH" = "preserved" ] && [ "$P_SRC" != "-" ]; then
    case ",$SRC_IPS," in
      *,"$P_SRC",*) SRC_MATCH="exact" ;;
    esac
  fi
fi

CAP_COUNT="-"
if [ "$DO_CAPTURE" = "1" ] && [ -r "$CAPTURE_PCAP" ]; then
  CAP_COUNT="$(tcpdump -n -r "$CAPTURE_PCAP" 2>/dev/null | wc -l | tr -d ' ')"
  [ -z "$CAP_COUNT" ] && CAP_COUNT="0"
fi

# ─────────────────────────────────────────────────────────────────────
# Report
# ─────────────────────────────────────────────────────────────────────
mark() { # $1=count $2=expected -> colored OK / PARTIAL / FAIL tag
  if [ "$1" -ge "$2" ]; then printf "%s[OK]%s" "$C_GRN" "$C_RST"
  elif [ "$1" -gt 0 ]; then printf "%s[PARTIAL]%s" "$C_YEL" "$C_RST"
  else printf "%s[FAIL]%s" "$C_RED" "$C_RST"; fi
}

echo
printf "%s── Warren port-forwarding deep test ──%s\n" "$C_BOLD" "$C_RST"
printf "  Protocol         : %s\n" "$PROTO_UC"
if [ "$MODE" = "connect" ]; then
  printf "  Local listener   : %s:%s  (live app, %s)\n" "0.0.0.0" "$LISTEN_PORT" "${HOLDER:-not identified}"
else
  printf "  Local listener   : 0.0.0.0:%s  (this script)\n" "$LISTEN_PORT"
fi
printf "  Probe target     : %s:%s  (exit public)\n" "$TARGET_IP" "$EXTERNAL_PORT"
printf "  External prober  : %s%s\n" "$SSH_DEST" "${SSH_PORT:+ (port $SSH_PORT)}"
printf "  Probes           : %s   payload %sB   timeout %ss\n" "$COUNT" "$PAYLOAD_SIZE" "$PROBE_TIMEOUT"
echo

if [ "$MODE" = "connect" ]; then
  printf "  %sReachability%s  (prober ─▶ exit DNAT ─▶ tunnel ─▶ live app)\n" "$C_CYN" "$C_RST"
  printf "    handshakes     : %s/%s   %s\n" "$P_ECHO" "$COUNT" "$(mark "$P_ECHO" "$COUNT")"
  printf "    RTT (ms)       : min %s / avg %s / max %s\n" "$R_MIN" "$R_AVG" "$R_MAX"
else

printf "  %sInbound%s  (prober ─▶ exit DNAT ─▶ tunnel ─▶ here)\n" "$C_CYN" "$C_RST"
printf "    delivered      : %s/%s   %s\n" "$INBOUND" "$COUNT" "$(mark "$INBOUND" "$COUNT")"
printf "    source IP seen : %s\n" "${SRC_IPS:-none}"
printf "    source IP      : %s" "$SRC_MATCH"
case "$SRC_MATCH" in
  exact)     printf "   (== prober socket %s, pure DNAT)\n" "$P_SRC" ;;
  preserved) printf "   (public source kept; prober reported %s, its own NAT rewrote it)\n" "$P_SRC" ;;
  snat)      printf "   %s(source == exit %s, the exit masquerades inbound traffic)%s\n" "$C_YEL" "$TARGET_IP" "$C_RST" ;;
  private)   printf "   %s(private source seen, tunnel-internal rewrite on the path)%s\n" "$C_YEL" "$C_RST" ;;
  *)         printf "\n" ;;
esac
[ "$STRAY" -gt 0 ] && printf "    stray packets  : %s (unrelated traffic on this port)\n" "$STRAY"
echo
printf "  %sReturn path%s  (here ─▶ tunnel ─▶ exit ─▶ prober)\n" "$C_CYN" "$C_RST"
printf "    echoes back    : %s/%s   %s\n" "$P_ECHO" "$COUNT" "$(mark "$P_ECHO" "$COUNT")"
printf "    payload intact : %s/%s   %s\n" "$P_PLOK" "$COUNT" "$(mark "$P_PLOK" "$COUNT")"
printf "    RTT (ms)       : min %s / avg %s / max %s\n" "$R_MIN" "$R_AVG" "$R_MAX"
fi
if [ "$DO_CAPTURE" = "1" ]; then
  echo
  printf "  %sCapture%s on %s : %s packet(s) matched %s port %s\n" \
         "$C_CYN" "$C_RST" "$IFACE" "$CAP_COUNT" "$PROTO" "$LISTEN_PORT"
fi

if [ "$VERBOSE" = "1" ]; then
  echo
  printf "  %s── prober output ──%s\n" "$C_BOLD" "$C_RST"
  sed 's/^/    /' "$PROBER_OUT" 2>/dev/null || true
  echo
  printf "  %s── listener records ──%s\n" "$C_BOLD" "$C_RST"
  sed 's/^/    /' "$RESULT_FILE" 2>/dev/null || true
fi

# ─────────────────────────────────────────────────────────────────────
# Verdict + actionable hints
# ─────────────────────────────────────────────────────────────────────
echo
RC=0
if [ "$MODE" = "connect" ]; then
  if [ "$P_ECHO" -ge "$COUNT" ]; then
    printf "  %s%sVERDICT: PASS%s, the forwarded port is open and the live app answers.\n" \
           "$C_BOLD" "$C_GRN" "$C_RST"
  else
    printf "  %s%sVERDICT: FAIL%s, only %s/%s handshake(s) completed.\n" \
           "$C_BOLD" "$C_RED" "$C_RST" "$P_ECHO" "$COUNT"
    echo  "    The mapping is live, so check that the app really listens on"
    echo  "    $LISTEN_PORT and that the granted public port is $EXTERNAL_PORT."
    RC=3
  fi
  echo
  exit "$RC"
fi
if [ "$INBOUND" -ge "$COUNT" ] && [ "$P_ECHO" -ge "$COUNT" ] && [ "$P_PLOK" -ge "$COUNT" ]; then
  printf "  %s%sVERDICT: PASS%s, port forwarding fully operational (bidirectional).\n" "$C_BOLD" "$C_GRN" "$C_RST"
  RC=0
elif [ "$INBOUND" -gt 0 ] && [ "$P_ECHO" -eq 0 ]; then
  printf "  %s%sVERDICT: PARTIAL%s, inbound forwarding WORKS, but the return path is broken.\n" "$C_BOLD" "$C_YEL" "$C_RST"
  echo  "    Hints: client-side egress firewall dropping replies, or the exit is"
  echo  "    not conntrack-restoring the reply path. The forward itself is fine."
  RC=2
elif [ "$INBOUND" -gt 0 ]; then
  printf "  %s%sVERDICT: PARTIAL%s, some probes lost (%s/%s inbound, %s/%s echoed).\n" \
         "$C_BOLD" "$C_YEL" "$C_RST" "$INBOUND" "$COUNT" "$P_ECHO" "$COUNT"
  echo  "    Hints: UDP loss/congestion, or RTT > --timeout. Retry with -c higher / -w larger."
  RC=2
else
  printf "  %s%sVERDICT: FAIL%s, no probe reached the local listener.\n" "$C_BOLD" "$C_RED" "$C_RST"
  echo  "    Check, in order:"
  echo  "      • Is port forwarding actually enabled & MAPPED right now?"
  echo  "      • Does --external-port match the port GRANTED by the exit (may differ"
  echo  "        from the one you requested)?"
  echo  "      • Is --target-ip the exit's PUBLIC IP, and the prober truly off-tunnel?"
  if [ "$DO_CAPTURE" = "1" ] && [ "$CAP_COUNT" != "-" ] && [ "$CAP_COUNT" -gt 0 ]; then
    echo  "      • Capture saw $CAP_COUNT packet(s) on $IFACE → packets ARRIVE but a LOCAL"
    echo  "        firewall drops them before the socket. Fix host/pf rules."
  else
    echo  "      • Re-run with --capture --iface <tunnel-if> to see if packets even arrive."
  fi
  RC=3
fi
echo

exit "$RC"
