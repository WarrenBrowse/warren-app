#!/usr/bin/env bash
# Build the Debug app for one product environment and run it on the currently
# booted iOS simulator, booting the default one first if none is running.
#
# Usage: scripts/dev/launch-ios.sh <--prod|--beta|--staging> [--logs]
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
IOS_DIR="$REPO_ROOT/ios"
BUNDLE_ID="com.warrenbrowse.vpn.ios"
APP_PATH="$IOS_DIR/build/Build/Products/Debug-iphonesimulator/WarrenVPN.app"
ENV_STAMP="$IOS_DIR/build/warren-product-env"

# shellcheck source=../utils/product-env.sh
source "$REPO_ROOT/scripts/utils/product-env.sh"

usage() {
    cat <<EOF
Build and launch the Warren iOS app on a simulator.

Usage: $(basename "$0") <--prod|--beta|--staging> [--logs]

Environment (required, no default: the API host is baked into the app bundle
at build time):
  --prod       Production build, the API host of the Debug configuration
  --beta       Beta build
  --staging    Staging build
  The WARREN_PRODUCT_ENV env var is used when no flag is given.

Options:
  --logs       Stream the app console into this terminal after launch
  -h, --help   Show this help

Unlike desktop and Android, iOS has a single bundle id for the three
environments, so they replace each other on the simulator. This script
uninstalls the app whenever the environment changes, otherwise the previous
environment's account and settings would carry into the new one.

Examples:
  $(basename "$0") --beta
  $(basename "$0") --prod --logs
EOF
}

ENV_FLAG=""
STREAM_LOGS=0
for arg in "$@"; do
    if warren_env_flag "$arg"; then
        ENV_FLAG="$WARREN_ENV_FLAG"
        continue
    fi
    case "$arg" in
        --logs) STREAM_LOGS=1 ;;
        -h|--help) usage; exit 0 ;;
        *)
            usage >&2
            printf '\nerror: unknown argument: %s\n' "$arg" >&2
            exit 1
            ;;
    esac
done
warren_env_require "$ENV_FLAG"

# The Xcode project carries one configuration per environment for prod
# (Debug) and staging, and none for beta, so the environment is applied as
# build-setting overrides on top of Debug instead: they win over the xcconfig
# and reach the app through WarrenREST's Info.plist. Two settings matter,
# and they move together: the hostname the REST client dials, and the
# bootstrap endpoint its address cache starts from (the iOS resolver has no
# system-DNS fallback, so it must be a live IP, and the one baked in the
# xcconfig only ever matches prod).
BUILD_SETTINGS=()
if [[ "$WARREN_PRODUCT_ENV" != "prod" ]]; then
    API_HOST="$(warren_env_api_host "$WARREN_PRODUCT_ENV")"
    API_IP="$(dig +short "$API_HOST" A | grep -Em1 '^[0-9]+(\.[0-9]+){3}$' || true)"
    if [[ -z "$API_IP" ]]; then
        printf 'error: %s does not resolve, so the %s bootstrap endpoint is unknown.\n' \
            "$API_HOST" "$WARREN_PRODUCT_ENV" >&2
        printf 'Check DNS (dig %s) and re-run.\n' "$API_HOST" >&2
        exit 1
    fi
    BUILD_SETTINGS=("API_HOST_NAME=$API_HOST" "API_ENDPOINT=$API_IP:443")
    echo "Environment: $WARREN_PRODUCT_ENV ($API_HOST, bootstrap $API_IP:443)"
else
    echo "Environment: prod"
fi

booted_count() { xcrun simctl list devices booted | grep -c "(Booted)" || true; }
available_count() { xcrun simctl list devices available | grep -cE '\([0-9A-F-]{36}\)' || true; }

if [[ "$(booted_count)" -eq 0 ]]; then
    if [[ "$(available_count)" -eq 0 ]]; then
        cat >&2 <<'EOF'
error: no iOS simulator exists on this machine.
Fix one of:
  - install an iOS simulator runtime: Xcode > Settings > Components (Platforms)
    or run: xcodebuild -downloadPlatform iOS
  - then create a device: Xcode > Window > Devices and Simulators > Simulators
    or run: xcrun simctl create "iPhone 17 Pro" "iPhone 17 Pro"
Then re-run this script.
EOF
        exit 1
    fi
    echo "No booted simulator, starting the default one..."
    open -a Simulator
    for _ in $(seq 1 30); do
        [[ "$(booted_count)" -gt 0 ]] && break
        sleep 2
    done
    if [[ "$(booted_count)" -eq 0 ]]; then
        cat >&2 <<'EOF'
error: the Simulator app did not boot a device within 60s.
Boot one manually and re-run this script:
  - Simulator menu: File > Open Simulator > iOS > <device>
  - or: xcrun simctl boot "iPhone 17 Pro"
EOF
        exit 1
    fi
fi

echo "Building WarrenVPN (Debug, simulator)..."
xcodebuild -project "$IOS_DIR/WarrenVPN.xcodeproj" \
    -scheme WarrenVPN \
    -configuration Debug \
    -destination 'generic/platform=iOS Simulator' \
    -derivedDataPath "$IOS_DIR/build" \
    ${BUILD_SETTINGS[@]+"${BUILD_SETTINGS[@]}"} \
    -quiet \
    build

if [[ "$(cat "$ENV_STAMP" 2>/dev/null)" != "$WARREN_PRODUCT_ENV" ]]; then
    echo "Environment changed, removing the previously installed app..."
    xcrun simctl uninstall booted "$BUNDLE_ID" >/dev/null 2>&1 || true
fi

echo "Installing on the booted simulator..."
xcrun simctl install booted "$APP_PATH"
printf '%s\n' "$WARREN_PRODUCT_ENV" > "$ENV_STAMP"

echo "Launching $BUNDLE_ID..."
if [[ "$STREAM_LOGS" -eq 1 ]]; then
    exec xcrun simctl launch --console-pty booted "$BUNDLE_ID"
else
    xcrun simctl launch booted "$BUNDLE_ID"
fi
