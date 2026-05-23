#!/usr/bin/env bash
# D.7 helper: build a signed Warren Android release APK/AAB.
#
# Canonical local-build path for the Play Store internal-test upload
# flow. Expects the four signing env vars to be set BEFORE invocation:
#
#   WARREN_KEYSTORE_PATH       absolute path to the .keystore / .jks file
#   WARREN_KEYSTORE_PASSWORD   keystore password
#   WARREN_KEY_ALIAS           alias of the signing key inside the keystore
#   WARREN_KEY_PASSWORD        password of the signing key
#
# Without all four env vars set the gradle build still runs, but the
# release artefact is UNSIGNED and cannot be uploaded to Play Console.
# The script aborts with a clear error in that case.
#
# Gradle daemon hazard: env vars are read at Gradle CONFIGURE time and
# a long-lived daemon's environment is frozen at daemon start. To
# avoid silently shipping an unsigned build when the daemon is warm,
# this script forces `--no-daemon` AND additionally forwards the four
# secrets as Gradle properties (`-Pwarren.keystore.*`) which bypass
# the daemon-env trap entirely.
#
# Outputs (relative to repo root, under android/):
#   APK : app/build/outputs/apk/prod/release/app-prod-release.apk
#   AAB : app/build/outputs/bundle/prodRelease/app-prod-release.aab
#
# Play Store internal-test upload procedure (manual, requires Google
# Play Console access — cannot be automated from a fresh checkout):
#   1. Open https://play.google.com/console -> Warren VPN -> Testing
#      -> Internal testing.
#   2. "Create new release". Upload the .aab from the path above.
#   3. Fill the release notes (free-form). Save -> Review release ->
#      Start rollout to Internal testing.
#   4. The internal-tester group ("Warren team") receives the new
#      build over the Play Store within ~15 minutes.
#
# Notes:
#   - The first upload must use the same signing key as the existing
#     Play Console "App signing key" config. If the keystore is fresh,
#     enrol it in Play App Signing first (Console -> Setup -> App
#     integrity) by uploading the .keystore and supplying the upload
#     key separately.
#   - The Play Store reviewers will reject the build if the
#     `targetSdk` is below the current Play minimum (see
#     `compile-sdk-major` in gradle/libs.versions.toml).

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ANDROID_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

missing=()
for var in WARREN_KEYSTORE_PATH WARREN_KEYSTORE_PASSWORD WARREN_KEY_ALIAS WARREN_KEY_PASSWORD; do
    if [[ -z "${!var:-}" ]]; then
        missing+=("$var")
    fi
done
if [[ ${#missing[@]} -gt 0 ]]; then
    echo "FAIL: missing signing env vars: ${missing[*]}" >&2
    echo "      Set all four to enable the warrenRelease signing config." >&2
    exit 2
fi
if [[ ! -f "$WARREN_KEYSTORE_PATH" ]]; then
    echo "FAIL: WARREN_KEYSTORE_PATH=$WARREN_KEYSTORE_PATH not found." >&2
    exit 2
fi

cd "$ANDROID_DIR"

# Forward signing secrets as Gradle properties + `--no-daemon` to
# bypass the daemon-env-freeze trap. Property lookups happen
# per-invocation (no caching), so the property path is robust against
# stale daemons; --no-daemon adds belt-and-braces.
PROPS=(
    "-Pwarren.keystore.path=$WARREN_KEYSTORE_PATH"
    "-Pwarren.keystore.password=$WARREN_KEYSTORE_PASSWORD"
    "-Pwarren.key.alias=$WARREN_KEY_ALIAS"
    "-Pwarren.key.password=$WARREN_KEY_PASSWORD"
)

echo "==> Gradle assemble (signed APK)"
./gradlew --no-daemon "${PROPS[@]}" :app:assembleProdRelease

echo "==> Gradle bundle (signed AAB for Play Store)"
./gradlew --no-daemon "${PROPS[@]}" :app:bundleProdRelease

APK_OUT="$ANDROID_DIR/app/build/outputs/apk/prod/release/app-prod-release.apk"
AAB_OUT="$ANDROID_DIR/app/build/outputs/bundle/prodRelease/app-prod-release.aab"

if [[ -f "$APK_OUT" ]]; then
    echo "APK : $APK_OUT ($(du -h "$APK_OUT" | cut -f1))"
fi
if [[ -f "$AAB_OUT" ]]; then
    echo "AAB : $AAB_OUT ($(du -h "$AAB_OUT" | cut -f1))"
fi

echo ""
echo "Build complete. To upload to Play Console internal testing, follow"
echo "the manual procedure documented in the header of this script."
