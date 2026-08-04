#!/usr/bin/env bash
# Build the debug APK of one product environment and run it on the connected
# Android device or emulator, starting the first available AVD if nothing is
# connected.
#
# Usage: scripts/dev/launch-android.sh <--prod|--beta|--staging> [--logs]
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ANDROID_DIR="$REPO_ROOT/android"
ACTIVITY="com.warrenbrowse.vpn.app.MainActivity"

# shellcheck source=../utils/product-env.sh
source "$REPO_ROOT/scripts/utils/product-env.sh"

usage() {
    cat <<EOF
Build and launch the Warren Android app on a device or emulator.

Usage: $(basename "$0") <--prod|--beta|--staging> [--logs]

Environment (required, no default: the API host and the update channel are
compiled into the APK, and each one installs as its own app):
  --prod       Production build (com.warrenbrowse.vpn)
  --beta       Beta build       (com.warrenbrowse.vpn.beta)
  --staging    Staging build    (com.warrenbrowse.vpn.staging)
  The WARREN_PRODUCT_ENV env var is used when no flag is given.

Options:
  --logs       Stream the app logcat into this terminal after launch
  -h, --help   Show this help

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

case "$WARREN_PRODUCT_ENV" in
    prod)    APP_ID="com.warrenbrowse.vpn";         VARIANT="ProdDebug" ;;
    beta)    APP_ID="com.warrenbrowse.vpn.beta";    VARIANT="BetaDebug" ;;
    staging) APP_ID="com.warrenbrowse.vpn.staging"; VARIANT="StagingDebug" ;;
esac

SDK="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Library/Android/sdk}}"
ADB="$SDK/platform-tools/adb"
EMULATOR="$SDK/emulator/emulator"
[[ -x "$ADB" ]] || ADB="$(command -v adb)" || { echo "error: adb not found" >&2; exit 1; }

device_count() { "$ADB" devices | awk 'NR > 1 && $2 == "device"' | wc -l | tr -d ' '; }

if [[ "$(device_count)" -eq 0 ]]; then
    if [[ "$("$ADB" devices | awk 'NR > 1 && $2 == "unauthorized"' | wc -l | tr -d ' ')" -gt 0 ]]; then
        cat >&2 <<'EOF'
error: an Android device is connected but not authorized for debugging.
Unlock the device, accept the "Allow USB debugging?" prompt on its screen,
then re-run this script (check with: adb devices).
EOF
        exit 1
    fi
    AVD="$("$EMULATOR" -list-avds 2>/dev/null | head -n 1)"
    if [[ -z "$AVD" ]]; then
        cat >&2 <<'EOF'
error: no Android device connected and no emulator (AVD) to start.
Fix one of:
  - plug in a device with USB debugging enabled (Settings > Developer options),
    then check it shows as "device" in: adb devices
  - create an emulator: Android Studio > Tools > Device Manager > Create device
Then re-run this script.
EOF
        exit 1
    fi
    echo "No device connected, starting AVD $AVD..."
    nohup "$EMULATOR" -avd "$AVD" >/dev/null 2>&1 &
    "$ADB" wait-for-device
    echo "Waiting for the system to finish booting..."
    until [[ "$("$ADB" shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')" == "1" ]]; do
        sleep 2
    done
fi

echo "Building + installing WarrenVPN ($WARREN_PRODUCT_ENV debug)..."
# :app: scope matters: the bare task name would also install the e2e test app.
# The environment is passed explicitly rather than inferred from the task name,
# so the Rust datapath in the APK can only be the one this launcher asked for.
(cd "$ANDROID_DIR" && ./gradlew ":app:install$VARIANT" \
    "-Pwarren.app.build.productEnv=$WARREN_PRODUCT_ENV")

echo "Launching $APP_ID..."
"$ADB" shell am start -n "$APP_ID/$ACTIVITY"

if [[ "$STREAM_LOGS" -eq 1 ]]; then
    # pidof can lag right after am start, retry briefly before giving up
    PID=""
    for _ in $(seq 1 10); do
        PID="$("$ADB" shell pidof -s "$APP_ID" 2>/dev/null | tr -d '\r')"
        [[ -n "$PID" ]] && break
        sleep 1
    done
    if [[ -n "$PID" ]]; then
        exec "$ADB" logcat --pid "$PID"
    fi
    echo "warning: app process not found, falling back to full logcat" >&2
    exec "$ADB" logcat
fi
