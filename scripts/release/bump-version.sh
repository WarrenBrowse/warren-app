#!/usr/bin/env bash
#
# Bump the Warren app version across desktop and Android.
#
# Desktop and Android share the 1.x.y tag scheme and are bumped together here
# (Android bakes the desktop tag as its versionName). iOS is deliberately NOT
# touched: its MARKETING_VERSION is calendar-based (YYYY.N) and independent of
# the desktop 1.x scheme. The signed update-metadata pipeline
# (ci/build-version-metadata.py) hard-rejects a non-calendar iOS version, so
# forcing iOS to the desktop major.minor (as this script used to) set it to
# e.g. 1.11 and broke every desktop release's metadata signing. iOS is bumped
# in ios/Configurations/Version.xcconfig directly, on its own cadence.
#
# Usage:
#   scripts/release/bump-version.sh                bump the minor version
#                                                   (X.Y.Z -> X.(Y+1).0)
#   scripts/release/bump-version.sh <VERSION>      set an explicit version,
#                                                   e.g. 1.8.0 or 1.8.0-beta1
#   scripts/release/bump-version.sh --current      print the current
#                                                   version(s), no changes
#
# Files touched (nothing is committed; review with `git diff` and commit
# yourself):
#   dist-assets/desktop-product-version.txt   desktop version, the source of
#                                              truth read by mullvad-version
#                                              and the Electron GUI at build
#                                              time
#   dist-assets/android-version-name.txt      Android versionName
#   dist-assets/android-version-code.txt      Android versionCode, recomputed
#                                              via `cargo run --bin
#                                              mullvad-version versionCode`
#
# ios/Configurations/Version.xcconfig is intentionally NOT written (see above):
# iOS keeps its own calendar MARKETING_VERSION.
#
# Portable: bash 3.2+ (macOS default) and Linux. Requires `cargo` on PATH.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$REPO_ROOT"

source scripts/utils/log

DESKTOP_VERSION_FILE="dist-assets/desktop-product-version.txt"
ANDROID_VERSION_NAME_FILE="dist-assets/android-version-name.txt"
ANDROID_VERSION_CODE_FILE="dist-assets/android-version-code.txt"
IOS_XCCONFIG="ios/Configurations/Version.xcconfig"

die() { log_error "$*"; exit 1; }

usage() { sed -n '2,30p' "$0" | sed 's/^# \{0,1\}//'; }

# semver-ish: MAJOR.MINOR.PATCH with an optional -prerelease (e.g. 1.2.0-beta1).
valid_version() { printf '%s' "$1" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.]+)?$'; }

read_trimmed() { tr -d '[:space:]' < "$1"; }

xcconfig_value() { grep -E "^$1[[:space:]]*=" "$IOS_XCCONFIG" | sed -E 's/.*=[[:space:]]*//' | tr -d '[:space:]'; }

CURRENT_DESKTOP="$(read_trimmed "$DESKTOP_VERSION_FILE")"
CURRENT_ANDROID="$(read_trimmed "$ANDROID_VERSION_NAME_FILE")"
CURRENT_IOS_MARKETING="$(xcconfig_value MARKETING_VERSION)"
CURRENT_IOS_BUILD="$(xcconfig_value CURRENT_PROJECT_VERSION)"

show_current() {
    printf 'desktop  %s\n' "$CURRENT_DESKTOP"
    printf 'android  %s\n' "$CURRENT_ANDROID"
    printf 'ios      %s (build %s)\n' "$CURRENT_IOS_MARKETING" "$CURRENT_IOS_BUILD"
    if [[ "$CURRENT_DESKTOP" != "$CURRENT_ANDROID" ]]; then
        log_warn "desktop and android versions are currently out of sync"
    fi
}

case "${1:-}" in
    --current|-c|current|show|get)
        show_current
        exit 0
        ;;
    -h|--help)
        usage
        exit 0
        ;;
esac

if [[ $# -eq 0 ]]; then
    # Default action: bump the minor version off the desktop file (the
    # actively maintained source of truth), reset patch to 0, ignore any
    # prerelease suffix on the base being bumped.
    CORE="${CURRENT_DESKTOP%%-*}"
    IFS='.' read -r MAJOR MINOR _PATCH <<< "$CORE"
    NEW_VERSION="${MAJOR}.$((10#$MINOR + 1)).0"
elif [[ $# -eq 1 ]]; then
    NEW_VERSION="$1"
    valid_version "$NEW_VERSION" || die "invalid version '$NEW_VERSION' (expected e.g. 1.8.0 or 1.8.0-beta1)"
else
    die "too many arguments (try --help)"
fi

log_header "Bumping Warren app version: ${CURRENT_DESKTOP} -> ${NEW_VERSION}"

printf '%s\n' "$NEW_VERSION" > "$DESKTOP_VERSION_FILE"
printf '%s\n' "$NEW_VERSION" > "$ANDROID_VERSION_NAME_FILE"
log_info "desktop + android version -> $NEW_VERSION"

command -v cargo >/dev/null 2>&1 || die "cargo not found: needed to recompute the Android versionCode"
ANDROID_VERSION="$NEW_VERSION" cargo run -q --bin mullvad-version versionCode > "$ANDROID_VERSION_CODE_FILE"
log_info "android versionCode -> $(cat "$ANDROID_VERSION_CODE_FILE")"

log_info "ios left untouched (calendar MARKETING_VERSION $CURRENT_IOS_MARKETING, build $CURRENT_IOS_BUILD)"

log_success "Done. Review with 'git diff' and commit when ready."
