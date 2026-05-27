#!/usr/bin/env bash
set -euo pipefail

# ─────────────────────────────────────────────────────────────────────
# Warren VPN — Development launcher
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
  command -v "$1" >/dev/null 2>&1 || die "'$1' not found in PATH — install it first"
}

check_daemon_deps() {
  require_cmd cargo
  require_cmd protoc
}

check_app_deps() {
  require_cmd node
  require_cmd npm
  if [[ ! -d "$ELECTRON_DIR/node_modules/.vite" && ! -d "$ELECTRON_DIR/node_modules/vite" ]]; then
    warn "node_modules looks incomplete — running npm install"
    (cd "$ELECTRON_DIR" && npm install)
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
# Parse daemon flags (shared between foreground / background / both)
# ─────────────────────────────────────────────────────────────────────
DAEMON_BUILD_MODE="debug"
DAEMON_CARGO_FLAGS=()
DAEMON_RUN_FLAGS=("--disable-stdout-timestamps")

parse_daemon_flags() {
  DAEMON_BUILD_MODE="debug"
  DAEMON_CARGO_FLAGS=()
  DAEMON_RUN_FLAGS=("--disable-stdout-timestamps")

  while (( $# )); do
    case "$1" in
      --release)     DAEMON_BUILD_MODE="release"; DAEMON_CARGO_FLAGS+=(--release) ;;
      -v|-vv|-vvv)   DAEMON_RUN_FLAGS+=("$1") ;;
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
# stop — kill daemon tracked by PID file
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
    warn "Daemon did not exit after 5 s — sending SIGKILL"
    sudo kill -KILL "$pid" 2>/dev/null || true
  fi

  rm -f "$PID_FILE"
  ok "Daemon stopped"
}

# ─────────────────────────────────────────────────────────────────────
# daemon — build + run in foreground (standalone, exec-replaces shell)
# ─────────────────────────────────────────────────────────────────────
start_daemon_foreground() {
  parse_daemon_flags "$@"
  build_daemon
  ensure_sudo

  ok "Starting warren-daemon ($DAEMON_BUILD_MODE)"
  info "Socket: $RPC_SOCKET"
  info "Ctrl+C to stop"
  echo ""

  exec sudo "$DAEMON_BIN" "${DAEMON_RUN_FLAGS[@]}"
}

# ─────────────────────────────────────────────────────────────────────
# app — Electron with Vite hot-reload (standalone, exec-replaces shell)
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
# both — daemon (background) + app (foreground), unified lifecycle
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
      warn "App did not exit after 4 s — forcing"
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

  # ── Start daemon in background, log to file ──
  if pid_alive "$(read_pid_file)"; then
    warn "Daemon already running (PID $(read_pid_file)) — reusing"
  else
    : > "$DAEMON_LOG"
    sudo "$DAEMON_BIN" "${DAEMON_RUN_FLAGS[@]}" >> "$DAEMON_LOG" 2>&1 &
    DAEMON_PID=$!
    echo "$DAEMON_PID" > "$PID_FILE"

    sleep 1
    if ! pid_alive "$DAEMON_PID"; then
      err "Daemon exited immediately — last 20 lines:"
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

  printf "${C_BOLD}Warren VPN — dev status${C_RESET}\n"
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
${C_BOLD}Warren VPN — Development launcher${C_RESET}

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
