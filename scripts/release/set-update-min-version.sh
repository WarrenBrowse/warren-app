#!/usr/bin/env bash
# Manage the WARREN_UPDATE_MIN_VERSION GitHub Actions repository variable, or
# with --beta its beta-channel twin WARREN_UPDATE_MIN_VERSION_BETA.
#
# That variable becomes the manifest `minimum_supported_version` at the next
# release of its channel: clients older than it are hard-blocked by the
# forced-update screen (see docs/AUTO-UPDATE.md). Leave it unset for normal,
# optional updates. The two channels have independent version series, so each
# floor is read only by its own channel's release.
#
# Usage:
#   set-update-min-version.sh                # show the current value
#   set-update-min-version.sh <version>      # set it, e.g. 1.2.0 or 1.2.0-beta1
#   set-update-min-version.sh --unset        # remove it (disables forced update)
#   set-update-min-version.sh -y <version>   # skip the confirmation prompt
#   set-update-min-version.sh --beta ...     # the same, for the beta channel
#
# Env: WARREN_APP_REPO overrides the target repo (default WarrenBrowse/warren-app).
# Portable: bash 3.2+ (macOS) and Linux. Requires an authenticated `gh`.

set -euo pipefail

REPO="${WARREN_APP_REPO:-WarrenBrowse/warren-app}"
VAR="WARREN_UPDATE_MIN_VERSION"
ASSUME_YES=0
ACTION=""
VALUE=""

die() { printf 'error: %s\n' "$*" >&2; exit 1; }

usage() { sed -n '2,20p' "$0" | sed 's/^#\{1,\} \{0,1\}//'; }

while [ $# -gt 0 ]; do
  case "$1" in
    -y|--yes) ASSUME_YES=1 ;;
    --beta) VAR="WARREN_UPDATE_MIN_VERSION_BETA" ;;
    -h|--help) usage; exit 0 ;;
    --unset|--clear|unset|clear) ACTION="unset" ;;
    get|show) ACTION="get" ;;
    -*) die "unknown flag: $1 (try --help)" ;;
    *)
      [ -n "$ACTION" ] && [ "$ACTION" != "set" ] && die "unexpected argument: $1"
      ACTION="set"; VALUE="$1"
      ;;
  esac
  shift
done
[ -z "$ACTION" ] && ACTION="get"

command -v gh >/dev/null 2>&1 || die "gh (GitHub CLI) not found: https://cli.github.com"
gh auth status >/dev/null 2>&1 || die "gh is not authenticated. Run: gh auth login"

# Current value, or empty if the variable is unset. Never fails the script.
# gh prints the 404 body on stdout for an unset variable, so the exit status,
# not the output, decides.
current() {
  local value=""
  value="$(gh api "repos/$REPO/actions/variables/$VAR" --jq '.value' 2>/dev/null)" || value=""
  printf '%s' "$value"
}

# semver-ish: MAJOR.MINOR.PATCH with an optional -prerelease (e.g. 1.2.0-beta1).
valid_version() { printf '%s' "$1" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.]+)?$'; }

confirm() {
  [ "$ASSUME_YES" = "1" ] && return 0
  printf '%s [y/N] ' "$1"
  local reply=""
  read -r reply </dev/tty 2>/dev/null || reply=""
  case "$reply" in y|Y|yes|YES) return 0 ;; *) return 1 ;; esac
}

CUR="$(current)"

case "$ACTION" in
  get)
    if [ -n "$CUR" ]; then
      printf '%s = %s  (clients below this are hard-blocked)\n' "$VAR" "$CUR"
    else
      printf '%s is unset  (no forced update; updates are optional)\n' "$VAR"
    fi
    ;;
  set)
    valid_version "$VALUE" || die "invalid version '$VALUE' (expected e.g. 1.2.0 or 1.2.0-beta1)"
    printf 'Repo:    %s\n' "$REPO"
    printf 'Current: %s\n' "${CUR:-<unset>}"
    printf 'New:     %s\n' "$VALUE"
    printf 'This will HARD-BLOCK every client below %s at the next release.\n' "$VALUE"
    confirm "Set $VAR to $VALUE?" || die "aborted"
    gh variable set "$VAR" -R "$REPO" --body "$VALUE"
    printf 'done: %s = %s\n' "$VAR" "$VALUE"
    ;;
  unset)
    if [ -z "$CUR" ]; then printf '%s is already unset.\n' "$VAR"; exit 0; fi
    confirm "Remove $VAR (currently $CUR)? This disables forced updates." || die "aborted"
    gh variable delete "$VAR" -R "$REPO"
    printf 'done: %s removed.\n' "$VAR"
    ;;
esac
