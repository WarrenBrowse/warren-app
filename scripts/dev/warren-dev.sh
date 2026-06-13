#!/usr/bin/env bash
set -euo pipefail

# ─────────────────────────────────────────────────────────────────────
# Warren VPN, Development launcher
# Cross-platform (macOS / Linux), robust signal handling
#
# Usage:
#   ./warren-dev.sh daemon [--release] [-v..] [--no-log-file] [-- extra-args]
#   ./warren-dev.sh app
#   ./warren-dev.sh both   [--release] [-v..] [--no-log-file]
#   ./warren-dev.sh stop
#   ./warren-dev.sh status
# ─────────────────────────────────────────────────────────────────────

readonly SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
readonly ELECTRON_DIR="$REPO_ROOT/desktop/packages/mullvad-vpn"
readonly RPC_SOCKET="/var/run/warren-vpn"
readonly PID_FILE="/tmp/warren-daemon-dev.pid"
readonly DAEMON_LOG="/tmp/warren-daemon-dev.log"

# PIDs tracked for cleanup (global so traps can reach them)
DAEMON_PID=""
APP_PID=""
TAIL_PID=""
CLEANUP_DONE=0

# ─────────────────────────────────────────────────────────────────────
# Colors (disabled when not a tty)
# ─────────────────────────────────────────────────────────────────────
if [[ -t 1 ]]; then
  readonly C_RED='\033[0;31m'
  readonly C_GREEN='\033[0;32m'
  readonly C_YELLOW='\033[0;33m'
  readonly C_BLUE='\033[0;34m'
  readonly C_CYAN='\033[0;36m'
  readonly C_BOLD='\033[1m'
  readonly C_RESET='\033[0m'
else
  readonly C_RED='' C_GREEN='' C_YELLOW='' C_BLUE='' C_CYAN='' C_BOLD='' C_RESET=''
fi

info()  { printf "${C_BLUE}[info]${C_RESET}  %s\n" "$*"; }
ok()    { printf "${C_GREEN}[ok]${C_RESET}    %s\n" "$*"; }
warn()  { printf "${C_YELLOW}[warn]${C_RESET}  %s\n" "$*" >&2; }
err()   { printf "${C_RED}[error]${C_RESET} %s\n" "$*" >&2; }
die()   { err "$@"; exit 1; }

# ─────────────────────────────────────────────────────────────────────
# Dependency checks
# ─────────────────────────────────────────────────────────────────────
require_cmd() {
  command -v "$1" >/dev/null 2>&1 || die "'$1' not found in PATH, install it first"
}

check_daemon_deps() {
  require_cmd cargo
  require_cmd protoc
  ensure_quinn_fork
}

check_app_deps() {
  require_cmd node
  require_cmd npm
  if [[ ! -d "$ELECTRON_DIR/node_modules/.vite" && ! -d "$ELECTRON_DIR/node_modules/vite" ]]; then
    warn "node_modules looks incomplete, running npm install"
    (cd "$ELECTRON_DIR" && npm install)
  fi
}

# The daemon's [patch.crates-io] points at ../warren-core/vendor/quinn-fork,
# the local Quinn fork. That tree is gitignored in warren-core and rebuilt on
# demand from upstream Quinn plus the Warren patch, so a fresh clone is missing
# it and cargo would fail to resolve the patch with a cryptic path error. Seed
# it before the first cargo run, and re-seed it if the pinned content hash
# drifted (e.g. the pin was bumped upstream but this clone never re-ran setup,
# which would otherwise compile silently against stale Quinn).
ensure_quinn_fork() {
  local core_dir setup verify marker
  core_dir="$REPO_ROOT/../warren-core"
  if [[ ! -d "$core_dir" ]]; then
    die "warren-core not found at $core_dir
        The daemon depends on the local Quinn fork in warren-core
        (../warren-core/vendor/quinn-fork). Clone warren-core next to
        warren-app: git clone <warren-core-url> \"$core_dir\""
  fi
  core_dir="$(cd "$core_dir" && pwd)"
  setup="$core_dir/bench/scripts/setup-quinn-fork.sh"
  verify="$core_dir/bench/scripts/verify-quinn-fork-hash.sh"
  marker="$core_dir/vendor/quinn-fork/.warren-patch-applied"
  [[ -x "$setup" ]] || die "Missing $setup, is warren-core up to date?"

  if [[ ! -f "$marker" ]]; then
    info "Quinn fork absent, seeding ../warren-core/vendor/quinn-fork (one-time, clones upstream)…"
    (cd "$core_dir" && "$setup") || die "setup-quinn-fork.sh failed"
    ok "Quinn fork ready"
    return 0
  fi

  if [[ -x "$verify" ]] && ! (cd "$core_dir" && "$verify") >/dev/null 2>&1; then
    warn "Quinn fork drifted from the pinned hash, rebuilding it"
    rm -f "$marker"
    (cd "$core_dir" && "$setup") || die "setup-quinn-fork.sh failed"
    ok "Quinn fork re-seeded to the pinned version"
  fi
}

# ─────────────────────────────────────────────────────────────────────
# Process helpers
# ─────────────────────────────────────────────────────────────────────
pid_alive() {
  [[ -n "${1:-}" ]] && kill -0 "$1" 2>/dev/null
}

read_pid_file() {
  [[ -f "$PID_FILE" ]] && cat "$PID_FILE" 2>/dev/null || echo ""
}

# Recursively kill a process and all its children (portable macOS + Linux)
kill_tree() {
  local pid="${1:-}" sig="${2:-TERM}"
  [[ -z "$pid" ]] && return
  pid_alive "$pid" || return 0

  local children
  children="$(pgrep -P "$pid" 2>/dev/null)" || true
  for child in $children; do
    kill_tree "$child" "$sig"
  done
  kill "-$sig" "$pid" 2>/dev/null || true
}

# Pre-authenticate sudo so background processes don't hang waiting for password
ensure_sudo() {
  if ! sudo -n true 2>/dev/null; then
    info "Sudo required for daemon (socket $RPC_SOCKET)"
    sudo -v || die "Sudo authentication failed"
  fi
}

# ─────────────────────────────────────────────────────────────────────
# DNS safety net (macOS)
#
# While (re)connecting, the daemon points the system DNS at a local resolver
# in the 127/8 range. If it is killed before restoring the DNS (SIGKILL,
# crash), the system is left pointing at a dead resolver and name resolution
# breaks until a tunnel takes over again. We snapshot the DNS before launching
# the daemon and restore it if a loopback leak is detected after it exits.
# (The daemon itself also self-heals on its next startup; this covers the dev
# loop where the daemon is killed and not immediately restarted.)
# ─────────────────────────────────────────────────────────────────────
readonly OS_NAME="$(uname -s)"
readonly DNS_SNAPSHOT_FILE="/tmp/warren-dns-snapshot.txt"

is_macos() { [[ "$OS_NAME" == "Darwin" ]]; }

# True if the active system DNS points at a loopback (127.x / ::1) resolver.
dns_is_loopback() {
  is_macos || return 1
  scutil --dns 2>/dev/null \
    | grep -qE 'nameserver\[[0-9]+\][[:space:]]*:[[:space:]]*(127\.|::1)'
}

# Save each network service's current DNS, unless DNS is already poisoned.
snapshot_dns() {
  is_macos || return 0
  if dns_is_loopback; then
    warn "System DNS already points at a loopback resolver, skipping snapshot"
    return 0
  fi
  : > "$DNS_SNAPSHOT_FILE"
  local svc dns
  while IFS= read -r svc; do
    [[ "$svc" == \** ]] && continue   # skip disabled services (prefixed with '*')
    dns="$(networksetup -getdnsservers "$svc" 2>/dev/null | tr '\n' ' ')"
    printf '%s\t%s\n' "$svc" "$dns" >> "$DNS_SNAPSHOT_FILE"
  done < <(networksetup -listallnetworkservices 2>/dev/null | tail -n +2)
  info "DNS snapshot saved → $DNS_SNAPSHOT_FILE"
}

# Re-apply the snapshot taken by snapshot_dns (needs root for networksetup).
restore_dns_snapshot() {
  is_macos || return 0
  [[ -f "$DNS_SNAPSHOT_FILE" ]] || { warn "No DNS snapshot to restore"; return 1; }
  local svc dns
  while IFS=$'\t' read -r svc dns; do
    [[ -z "$svc" ]] && continue
    if [[ -z "${dns// /}" || "$dns" == *"any DNS Servers"* ]]; then
      sudo networksetup -setdnsservers "$svc" empty 2>/dev/null || true
    else
      # shellcheck disable=SC2086
      sudo networksetup -setdnsservers "$svc" $dns 2>/dev/null || true
    fi
  done < "$DNS_SNAPSHOT_FILE"
  sudo dscacheutil -flushcache 2>/dev/null || true
  sudo killall -HUP mDNSResponder 2>/dev/null || true
  ok "DNS restored from snapshot"
}

# Restore DNS only if the daemon left it pointing at a dead loopback resolver.
restore_dns_if_leaked() {
  is_macos || return 0
  if dns_is_loopback; then
    warn "Stale loopback DNS detected (daemon exited without restoring it), repairing"
    restore_dns_snapshot
  fi
}

# ─────────────────────────────────────────────────────────────────────
# Parse daemon flags (shared between foreground / background / both)
# ─────────────────────────────────────────────────────────────────────
DAEMON_BUILD_MODE="debug"
DAEMON_CARGO_FLAGS=()
DAEMON_RUN_FLAGS=("--disable-stdout-timestamps")

parse_daemon_flags() {
  DAEMON_BUILD_MODE="debug"
  DAEMON_CARGO_FLAGS=()
  # Default to INFO logging (-v): without it the daemon logs ERROR only,
  # which leaves the on-disk daemon.log blind to every tunnel state
  # transition (connect/teardown/pump death are INFO/WARN). The
  # 2026-06-11 torrent incident was undiagnosable from a default-level
  # log. Explicit -v/-vv/-vvv flags replace this default.
  DAEMON_RUN_FLAGS=("--disable-stdout-timestamps" "-v")

  while (( $# )); do
    case "$1" in
      --release)     DAEMON_BUILD_MODE="release"; DAEMON_CARGO_FLAGS+=(--release) ;;
      -v|-vv|-vvv)   DAEMON_RUN_FLAGS=("--disable-stdout-timestamps" "$1") ;;
      --no-log-file) DAEMON_RUN_FLAGS+=(--disable-log-to-file) ;;
      --)            shift; DAEMON_RUN_FLAGS+=("$@"); break ;;
      *)             die "Unknown daemon flag: $1" ;;
    esac
    shift
  done
}

build_daemon() {
  check_daemon_deps
  info "Building warren-daemon ($DAEMON_BUILD_MODE)…"
  cargo build --bin warren-daemon "${DAEMON_CARGO_FLAGS[@]}" \
    --manifest-path "$REPO_ROOT/Cargo.toml"

  DAEMON_BIN="$REPO_ROOT/target/$DAEMON_BUILD_MODE/warren-daemon"
  [[ -x "$DAEMON_BIN" ]] || die "Binary not found: $DAEMON_BIN"
}

# ─────────────────────────────────────────────────────────────────────
# stop, kill daemon tracked by PID file
# ─────────────────────────────────────────────────────────────────────
stop_daemon() {
  local pid
  pid="$(read_pid_file)"
  if [[ -z "$pid" ]] || ! pid_alive "$pid"; then
    info "No daemon running (PID file absent or stale)"
    rm -f "$PID_FILE"
    return 0
  fi

  info "Stopping warren-daemon (PID $pid)…"
  sudo kill -TERM "$pid" 2>/dev/null || true

  local waited=0
  while pid_alive "$pid" && (( waited < 10 )); do
    sleep 0.5
    (( waited++ ))
  done

  if pid_alive "$pid"; then
    warn "Daemon did not exit after 5 s, sending SIGKILL"
    sudo kill -KILL "$pid" 2>/dev/null || true
  fi

  rm -f "$PID_FILE"

  # If the daemon died before restoring the system DNS, repair it now.
  restore_dns_if_leaked

  ok "Daemon stopped"
}

# ─────────────────────────────────────────────────────────────────────
# daemon, build + run in foreground (standalone, exec-replaces shell)
# ─────────────────────────────────────────────────────────────────────
start_daemon_foreground() {
  parse_daemon_flags "$@"
  build_daemon
  ensure_sudo

  ok "Starting warren-daemon ($DAEMON_BUILD_MODE)"
  info "Socket: $RPC_SOCKET"
  info "Ctrl+C to stop"
  echo ""

  # WARREN_USE_PLAINTEXT_STORAGE=1 forces the daemon to skip the
  # macOS System Keychain / Windows DPAPI backend and persist the
  # mnemonic as a `0o600 root:root` plaintext file in `<settings_dir>/secrets/`.
  # On unsigned dev builds, the System Keychain triggers a macOS
  # Authorization Services prompt at every launch (because the binary
  # hash changes on each `cargo build`). The env var keeps the dev
  # loop friction-free. Release builds with a stable Developer ID
  # signature should leave this unset.
  # Snapshot DNS and arm a restore-on-exit guard. We run the daemon as a
  # foreground child (NOT `exec`) so the EXIT trap can fire after it stops and
  # repair the DNS if the daemon was killed before it could restore it itself.
  snapshot_dns
  trap restore_dns_if_leaked EXIT

  sudo -E env WARREN_USE_PLAINTEXT_STORAGE=1 \
    "$DAEMON_BIN" "${DAEMON_RUN_FLAGS[@]}" || true
}

# ─────────────────────────────────────────────────────────────────────
# app, Electron with Vite hot-reload (standalone, exec-replaces shell)
# ─────────────────────────────────────────────────────────────────────
start_app() {
  check_app_deps

  if [[ ! -S "$RPC_SOCKET" ]]; then
    warn "Daemon does not seem to be running ($RPC_SOCKET absent)"
    warn "The app will start but won't connect until the daemon is up"
    warn "Hint: ./warren-dev.sh both"
    echo ""
  fi

  ok "Starting Electron app (Vite hot-reload)"
  info "Ctrl+C to stop"
  echo ""

  cd "$ELECTRON_DIR"
  exec npm run develop
}

# ─────────────────────────────────────────────────────────────────────
# both, daemon (background) + app (foreground), unified lifecycle
# ─────────────────────────────────────────────────────────────────────
cleanup_both() {
  (( CLEANUP_DONE )) && return
  CLEANUP_DONE=1

  echo ""
  info "Shutting down…"

  # 1. Kill daemon log tailer
  if [[ -n "$TAIL_PID" ]] && pid_alive "$TAIL_PID"; then
    kill "$TAIL_PID" 2>/dev/null || true
    wait "$TAIL_PID" 2>/dev/null || true
  fi

  # 2. Kill Electron app + all children (vite, electron, esbuild…)
  if [[ -n "$APP_PID" ]] && pid_alive "$APP_PID"; then
    info "Stopping app (PID $APP_PID)…"
    kill_tree "$APP_PID" TERM
    local i=0
    while pid_alive "$APP_PID" && (( i < 8 )); do
      sleep 0.5; (( i++ ))
    done
    if pid_alive "$APP_PID"; then
      warn "App did not exit after 4 s, forcing"
      kill_tree "$APP_PID" KILL
    fi
    wait "$APP_PID" 2>/dev/null || true
    ok "App stopped"
  fi

  # 3. Kill daemon (runs as root → needs sudo)
  stop_daemon

  ok "All stopped"
}

start_both() {
  parse_daemon_flags "$@"
  build_daemon
  check_app_deps

  # Pre-auth sudo before going into background mode
  ensure_sudo

  trap cleanup_both EXIT INT TERM HUP

  # Snapshot DNS before the daemon touches it; cleanup_both → stop_daemon
  # restores it if the daemon is killed before it can do so itself.
  snapshot_dns

  # ── Start daemon in background, log to file ──
  if pid_alive "$(read_pid_file)"; then
    warn "Daemon already running (PID $(read_pid_file)), reusing"
  else
    : > "$DAEMON_LOG"
    # See `start_daemon_foreground` for the rationale behind
    # `WARREN_USE_PLAINTEXT_STORAGE=1` in dev mode.
    sudo -E env WARREN_USE_PLAINTEXT_STORAGE=1 \
      "$DAEMON_BIN" "${DAEMON_RUN_FLAGS[@]}" >> "$DAEMON_LOG" 2>&1 &
    DAEMON_PID=$!
    echo "$DAEMON_PID" > "$PID_FILE"

    sleep 1
    if ! pid_alive "$DAEMON_PID"; then
      err "Daemon exited immediately, last 20 lines:"
      tail -20 "$DAEMON_LOG" >&2
      exit 1
    fi
    ok "Daemon running (PID $DAEMON_PID)"
  fi

  # ── Stream daemon logs with [daemon] prefix in background ──
  tail -f "$DAEMON_LOG" 2>/dev/null \
    | awk '{printf "\033[33m[daemon]\033[0m %s\n", $0; fflush(stdout)}' &
  TAIL_PID=$!

  echo ""
  ok "Starting Electron app (Vite hot-reload)"
  info "Ctrl+C stops both daemon and app"
  echo ""

  # ── Start app as a child (NOT exec) so the trap stays alive ──
  cd "$ELECTRON_DIR"
  npm run develop &
  APP_PID=$!

  # Wait for app to exit (Ctrl+C interrupts wait → trap fires → cleanup)
  wait "$APP_PID" 2>/dev/null || true
}

# ─────────────────────────────────────────────────────────────────────
# status
# ─────────────────────────────────────────────────────────────────────
show_status() {
  local pid
  pid="$(read_pid_file)"

  printf "${C_BOLD}Warren VPN, dev status${C_RESET}\n"
  echo ""

  if [[ -n "$pid" ]] && pid_alive "$pid"; then
    ok "Daemon:  running (PID $pid)"
  else
    warn "Daemon:  not running"
  fi

  if [[ -S "$RPC_SOCKET" ]]; then
    ok "Socket:  $RPC_SOCKET (exists)"
  else
    warn "Socket:  $RPC_SOCKET (missing)"
  fi

  if pgrep -f "vite.*mullvad-vpn\|electron.*mullvad-vpn" >/dev/null 2>&1; then
    ok "App:     running"
  else
    info "App:     not running"
  fi
}

# ─────────────────────────────────────────────────────────────────────
# Usage
# ─────────────────────────────────────────────────────────────────────
usage() {
  cat <<EOF
${C_BOLD}Warren VPN, Development launcher${C_RESET}

${C_GREEN}Usage:${C_RESET}
  $(basename "$0") <command> [options]

${C_GREEN}Commands:${C_RESET}
  ${C_BOLD}daemon${C_RESET}   Build & run the Rust daemon (foreground, with sudo)
  ${C_BOLD}app${C_RESET}      Start the Electron app only (Vite hot-reload)
  ${C_BOLD}both${C_RESET}     Daemon + app, unified lifecycle (Ctrl+C stops both)
  ${C_BOLD}stop${C_RESET}     Stop a background daemon
  ${C_BOLD}status${C_RESET}   Show running components

${C_GREEN}Daemon options:${C_RESET}
  --release        Build in release mode
  -v / -vv / -vvv  Increase log verbosity
  --no-log-file    Log to stdout only (no file)
  -- <args>        Pass extra args to warren-daemon

${C_GREEN}Examples:${C_RESET}
  $(basename "$0") daemon -vv
  $(basename "$0") app
  $(basename "$0") both --release -v
  $(basename "$0") stop
EOF
}

# ─────────────────────────────────────────────────────────────────────
# Main
# ─────────────────────────────────────────────────────────────────────
main() {
  [[ $# -lt 1 ]] && { usage; exit 0; }

  local cmd="$1"; shift

  case "$cmd" in
    daemon)  start_daemon_foreground "$@" ;;
    app)     start_app ;;
    both)    start_both "$@" ;;
    stop)    stop_daemon ;;
    status)  show_status ;;
    -h|--help|help) usage ;;
    *)       die "Unknown command: $cmd (try --help)" ;;
  esac
}

main "$@"
