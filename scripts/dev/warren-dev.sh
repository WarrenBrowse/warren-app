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
# Which product environment to build and run, set by --prod/--beta/--staging
# on any command (the env var is honoured as the default). The same selector
# drives the Rust build (warren-product-env's build.rs) and the Electron build
# (vite.config.ts), so one choice moves the API host, the update channel and
# the runtime paths together. Prod stays the default.
WARREN_PRODUCT_ENV="${WARREN_PRODUCT_ENV:-prod}"
PRODUCT_DIR=""
RPC_SOCKET=""
PID_FILE=""
DAEMON_LOG=""

# Runtime paths are per-environment by design (mullvad-paths pins that beta can
# never alias prod), so hardcoding prod's would have this launcher watch the
# wrong socket and report a beta daemon as down.
resolve_product_env() {
  case "$WARREN_PRODUCT_ENV" in
    prod)    PRODUCT_DIR="warren-vpn" ;;
    beta)    PRODUCT_DIR="warren-vpn-beta" ;;
    staging) PRODUCT_DIR="warren-vpn-staging" ;;
    *) die "environment must be prod, beta or staging, got: $WARREN_PRODUCT_ENV" ;;
  esac
  RPC_SOCKET="/var/run/$PRODUCT_DIR"
  PID_FILE="/tmp/$PRODUCT_DIR-daemon-dev.pid"
  DAEMON_LOG="/tmp/$PRODUCT_DIR-daemon-dev.log"
}

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
  ensure_engine_siblings
}

check_app_deps() {
  require_cmd node
  require_cmd npm
  if [[ ! -d "$ELECTRON_DIR/node_modules/.vite" && ! -d "$ELECTRON_DIR/node_modules/vite" ]]; then
    warn "node_modules looks incomplete, running npm install"
    (cd "$ELECTRON_DIR" && npm install)
  fi
  ensure_electron_binary
}

# The `electron` npm package downloads its platform binary in a postinstall
# step (node_modules/electron/install.js, which writes path.txt and unpacks
# dist/). ROOT CAUSE of the recurring "Electron failed to install correctly":
# desktop/.npmrc sets `ignore-scripts=true` (a deliberate Mullvad security
# choice), so EVERY `npm install` / `npm ci` skips that postinstall and the
# binary is never fetched automatically. Nothing else in the repo installs it
# explicitly, so a fresh install always leaves Electron broken until repaired
# by hand. (An inherited ELECTRON_SKIP_BINARY_DOWNLOAD, an interrupted/partial
# install, or a proxy hiccup produce the same state.) Fix it for good here:
# detect the missing binary and run the installer DIRECTLY with `node`
# (bypassing the npm ignore-scripts gate), forcing the download, before every
# dev launch. Idempotent and a no-op once the binary is in place.
ensure_electron_binary() {
  local pkg=""
  local candidate
  for candidate in "$REPO_ROOT/desktop/node_modules/electron" \
                   "$ELECTRON_DIR/node_modules/electron"; do
    if [[ -d "$candidate" ]]; then
      pkg="$candidate"
      break
    fi
  done
  # Package absent entirely: the node_modules check above (npm install) owns
  # that case; nothing to repair here.
  [[ -n "$pkg" ]] || return 0

  # "Correctly installed" == path.txt names a binary that actually exists
  # under dist/ (this mirrors electron's own getElectronPath()).
  if [[ -f "$pkg/path.txt" ]]; then
    local rel
    rel="$(cat "$pkg/path.txt" 2>/dev/null)"
    if [[ -n "$rel" && -e "$pkg/dist/$rel" ]]; then
      return 0
    fi
  fi

  warn "Electron binary missing or incomplete, re-running its installer"
  # Repair path: force the download even if the caller exported the skip flag
  # (that flag is the most common cause of the breakage in the first place).
  if [[ -f "$pkg/install.js" ]] \
     && (cd "$pkg" && env -u ELECTRON_SKIP_BINARY_DOWNLOAD node install.js); then
    ok "Electron binary installed"
  else
    die "Electron binary install failed (network/proxy?). Manual fix:
        rm -rf \"$pkg\" && (cd \"$ELECTRON_DIR\" && npm install)"
  fi
}

# The daemon builds against three sibling repos it path-deps: the warrenguard
# engine (datapath), warren-sdk-rs (account client + identity), and
# warren-contract (SS58 + /v1 DTOs). cargo fails with a cryptic "path does not
# exist" if any is missing, so check up front. The quinn fork needs no local
# setup: it is the WarrenBrowse/warren-quinn git-dep pinned by tag in the root
# [patch.crates-io], fetched by cargo like any other git dep.
ensure_engine_siblings() {
  local sib
  for sib in warrenguard warren-sdk-rs warren-contract; do
    [[ -d "$REPO_ROOT/../$sib/.git" ]] || die "$sib not found at $REPO_ROOT/../$sib
        The daemon path-deps this sibling. Clone it next to warren-app, or run
        the workspace onboarding (mani sync)."
  done
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
# Network safety net (macOS + Linux)
#
# A daemon killed before it can tear down (SIGKILL, crash, hard Ctrl+C) leaves
# the host without working name resolution, all repaired after it exits by
# `repair_network_if_leaked`.
#
# macOS:
#   1. DNS pinned at a resolver the daemon set and that dies with it: a loopback
#      (127/8, ::1) or a Warren tunnel resolver (10.64.0.0/10 e.g. 10.66.0.1,
#      the `fdcc:` ULA e.g. fdcc:f:1::1), left in the Setup layer. We snapshot
#      the DNS before launch and restore it (`networksetup ... empty` clears the
#      Setup override so the box's own RA/DHCP resolver takes over again).
#   2. A leaked IPv6 split-default (`::/1`+`8000::/1` via a now-gone `utun`) that
#      blackholes all native IPv6. On a network whose ONLY resolver is reached
#      over IPv6 (RA/RDNSS), that kills DNS while IPv4 still pings, until reboot.
#   3. The PRIMARY interface's scoped IPv6 default route dropped on teardown
#      (dual-homed box: the tunnel restored only the dial interface's default).
#      The primary-scoped DNS resolver then has no route via its interface and
#      mDNSResponder stops resolving.
# Linux:
#   4. The static /etc/resolv.conf backend leaves resolv.conf on the dead tunnel
#      resolver with the original saved to /etc/resolv.conf.mullvadbackup.
#      (systemd-resolved / NetworkManager / resolvconf backends are per-link or
#      self-restoring and leave no persistent leak.)
# (The daemon self-heals on its next startup too; this covers the dev loop where
# it is killed and not immediately restarted.)
# ─────────────────────────────────────────────────────────────────────
readonly OS_NAME="$(uname -s)"
readonly DNS_SNAPSHOT_FILE="/tmp/warren-dns-snapshot.txt"
# talpid's static-resolv.conf backend (used when /etc/resolv.conf is a plain
# file, i.e. no systemd-resolved/NM/resolvconf) backs the original up here.
readonly LINUX_RESOLV_CONF="/etc/resolv.conf"
readonly LINUX_RESOLV_BACKUP="/etc/resolv.conf.mullvadbackup"

is_macos() { [[ "$OS_NAME" == "Darwin" ]]; }
is_linux() { [[ "$OS_NAME" == "Linux" ]]; }

# True if the active system DNS points at a resolver the daemon leaves behind on
# an unclean exit and that dies with it: a loopback (127.x / ::1) OR a Warren
# tunnel resolver. Warren's tunnel DNS is the Mullvad-style CGNAT range
# 10.64.0.0/10 (e.g. 10.66.0.1) for v4 and the `fdcc:` ULA (e.g. fdcc:f:1::1)
# for v6 - distinct from the box's own `fd0f:` resolver. macOS ignores
# /etc/resolv.conf (editing it does nothing), so `scutil --dns` is the only
# truth; the daemon's leak sits in the Setup layer (which overrides State), and
# `restore_dns_snapshot`'s `networksetup ... empty` clears exactly that layer.
dns_is_leaked() {
  is_macos || return 1
  scutil --dns 2>/dev/null \
    | grep -qE 'nameserver\[[0-9]+\][[:space:]]*:[[:space:]]*(127\.|::1|10\.(6[4-9]|[7-9][0-9]|1[01][0-9]|12[0-7])\.|fdcc:)'
}

# Save each network service's current DNS, unless DNS is already poisoned.
snapshot_dns() {
  is_macos || return 0
  if dns_is_leaked; then
    warn "System DNS already points at a leaked resolver, skipping snapshot"
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

# Delete any Warren IPv6 split-default half (`::/1` / `8000::/1`) the daemon
# left behind pointing at a now-gone `utun`. Such a dangling half blackholes
# ALL native IPv6; on a network whose only DNS resolver is reached over IPv6
# (RA/RDNSS, e.g. a Freebox handing out `fd0f:…::1`), that silently kills name
# resolution while IPv4 still pings ("ping IP ok, DNS dead") until reboot. This
# mirrors `warrenguard_route_split::default_route_split_macos::force_cleanup_all_v6`
# as a dev-loop backstop for when the daemon was hard-killed before its own
# teardown ran. Ownership-scoped: only a half egressing a `utun` that no longer
# exists is reclaimed, so a co-resident VPN's live split is never touched.
repair_leaked_v6_split() {
  is_macos || return 0
  local net iface repaired=0
  for net in "::/1" "8000::/1"; do
    iface="$(route -n get -inet6 "$net" 2>/dev/null | awk '/interface:/{print $2; exit}')"
    [[ "$iface" == utun* ]] || continue          # not a tunnel half → not ours
    ifconfig "$iface" >/dev/null 2>&1 && continue # utun still alive → leave it
    warn "Stale Warren IPv6 split-default route $net via gone $iface, deleting"
    sudo route -n delete -inet6 -net "$net" >/dev/null 2>&1 || true
    repaired=1
  done
  if (( repaired )); then
    sudo dscacheutil -flushcache 2>/dev/null || true
    sudo killall -HUP mDNSResponder 2>/dev/null || true
    ok "Repaired leaked IPv6 split-default routing"
  fi
}

# Restore the primary interface's scoped IPv6 default route if the daemon's
# teardown dropped it. THE observed root cause on a dual-homed macOS box: the
# tunnel captures the default route and on teardown restores only the dial
# interface's scoped default (e.g. en0), losing the PRIMARY service's one
# (e.g. en1/Wi-Fi). Because the system DNS resolver is scoped to the primary
# interface, it then has no route via that interface: `scutil --dns` marks it
# Not Reachable and mDNSResponder refuses to query it, so ALL name resolution
# dies even though `ping`/`dig` still work unscoped via the surviving default.
# Deterministic repair: re-add the scoped default from SC's own PrimaryInterface
# + Router (the exact route a healthy dual-homed box carries). Acts only on the
# bug's fingerprint (primary interface has no v6 default while another interface
# does), so a healthy single-homed box is never touched.
repair_stranded_primary_v6_default() {
  is_macos || return 0
  local pif router
  pif="$(echo 'show State:/Network/Global/IPv6' | scutil 2>/dev/null | awk '/PrimaryInterface/{print $3}')"
  router="$(echo 'show State:/Network/Global/IPv6' | scutil 2>/dev/null | awk '/Router/{print $3}')"
  [[ -n "$pif" && -n "$router" ]] || return 0
  # Already have a v6 default via the primary interface → healthy, nothing to do.
  netstat -rn -f inet6 2>/dev/null | grep -qE "^default[[:space:]].*%${pif}[[:space:]]" && return 0
  # Only act on the bug's fingerprint: a default survives via another interface.
  netstat -rn -f inet6 2>/dev/null | grep -qE "^default[[:space:]]" || return 0
  [[ "$router" == *%* ]] || router="${router}%${pif}"
  warn "Primary interface $pif lost its IPv6 default route (daemon teardown), restoring via $router"
  sudo route -n add -inet6 default -ifscope "$pif" "$router" >/dev/null 2>&1 || true
  sudo dscacheutil -flushcache 2>/dev/null || true
  sudo killall -HUP mDNSResponder 2>/dev/null || true
  ok "Restored primary-interface IPv6 default route"
}

# Linux: talpid's static /etc/resolv.conf backend (used only when resolv.conf is
# a plain file, i.e. no systemd-resolved/NM/resolvconf) backs the original up to
# /etc/resolv.conf.mullvadbackup and overwrites resolv.conf with the tunnel DNS.
# A hard kill skips its reset(), leaving resolv.conf pointing at the dead tunnel
# resolver with the backup still on disk. Restore it (the daemon does the same
# via restore_from_backup() on its next startup; this covers the dev loop where
# it is not restarted). The other backends are per-link or self-restoring and
# leave no such file, so the backup's presence is the exact static-backend
# signature. We do NOT match `127.` here: on Linux that is the legitimate
# systemd-resolved stub (127.0.0.53), not a leak.
repair_linux_dns_leak() {
  is_linux || return 0
  [[ -f "$LINUX_RESOLV_BACKUP" ]] || return 0
  if grep -qE '^[[:space:]]*nameserver[[:space:]]+(10\.(6[4-9]|[7-9][0-9]|1[01][0-9]|12[0-7])\.|fdcc:)' \
       "$LINUX_RESOLV_CONF" 2>/dev/null; then
    warn "Stale Warren tunnel DNS in $LINUX_RESOLV_CONF (daemon killed before restore), restoring backup"
    if sudo cp -f "$LINUX_RESOLV_BACKUP" "$LINUX_RESOLV_CONF" 2>/dev/null; then
      sudo rm -f "$LINUX_RESOLV_BACKUP" 2>/dev/null || true
      ok "Restored $LINUX_RESOLV_CONF from backup"
    fi
  fi
}

# Repair the ways a killed daemon can leave the host without working name
# resolution. macOS: (1) DNS pinned at a dead resolver it set (loopback OR a
# Warren tunnel resolver like 10.66.0.1 / fdcc:f:1::1, in the Setup layer),
# (2) a leaked IPv6 split-default blackholing the box's IPv6-only resolver,
# (3) the primary interface's scoped IPv6 default route dropped on teardown.
# Linux: the static-resolv.conf backend leaves resolv.conf on the dead tunnel
# resolver with its backup on disk. All safe no-ops when nothing leaked.
repair_network_if_leaked() {
  if is_macos; then
    if dns_is_leaked; then
      warn "Stale DNS detected (daemon exited without restoring its resolver), repairing"
      restore_dns_snapshot
    fi
    repair_leaked_v6_split
    repair_stranded_primary_v6_default
  elif is_linux; then
    repair_linux_dns_leak
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
  WARREN_PRODUCT_ENV="$WARREN_PRODUCT_ENV" \
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

  # If the daemon died before restoring the network (dead loopback DNS, a leaked
  # IPv6 split-default, or a stranded primary scoped default), repair it now.
  repair_network_if_leaked

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
  info "Environment: $WARREN_PRODUCT_ENV"
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
  trap repair_network_if_leaked EXIT

  sudo -E env WARREN_USE_PLAINTEXT_STORAGE=1 \
    WARREN_PRODUCT_ENV="$WARREN_PRODUCT_ENV" \
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
  export WARREN_PRODUCT_ENV
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
  export WARREN_PRODUCT_ENV
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

${C_GREEN}Environment (accepted on every command, default prod):${C_RESET}
  --prod           Production build (default)
  --beta           Beta build
  --staging        Staging build
  Selects the API host, the update channel and the runtime paths together.
  Each environment has its own socket and settings, so a beta daemon and a
  prod one can run side by side without seeing each other. The
  WARREN_PRODUCT_ENV env var sets the default when no flag is given.

${C_GREEN}Examples:${C_RESET}
  $(basename "$0") daemon -vv
  $(basename "$0") app
  $(basename "$0") both --release -v
  $(basename "$0") both --beta
  $(basename "$0") status --beta
  $(basename "$0") stop --beta
EOF
}

# ─────────────────────────────────────────────────────────────────────
# Main
# ─────────────────────────────────────────────────────────────────────
main() {
  # The environment flags are accepted on EVERY command and anywhere in the
  # line, so `stop --beta` and `--beta stop` both work. Everything after a
  # bare `--` is the daemon's own passthrough and is left untouched.
  local args=() passthrough=0
  local a
  for a in "$@"; do
    if [[ $passthrough -eq 0 ]]; then
      case "$a" in
        --prod)    WARREN_PRODUCT_ENV=prod;    continue ;;
        --beta)    WARREN_PRODUCT_ENV=beta;    continue ;;
        --staging) WARREN_PRODUCT_ENV=staging; continue ;;
        --)        passthrough=1 ;;
      esac
    fi
    args+=("$a")
  done
  resolve_product_env
  set -- ${args[@]+"${args[@]}"}

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
