#!/usr/bin/env bash
# Refuses an APK that is unsigned and, when given the expected fingerprint
# file, one whose signer is not the key every published beta install carries.
#
# Usage: ci/verify-android-signer.sh <apk> [expected-sha256-file]
#
# Without a keystore the release workflow signs the beta with the CI runner's
# AGP debug keystore (android/docs/BuildInstructions.md, "Release build
# without a keystore"). AGP generates a fresh ~/.android/debug.keystore
# wherever it finds none, so a runner rebuilt on another HOME would sign a
# green build with a new key, and every existing install would answer
# INSTALL_FAILED_UPDATE_INCOMPATIBLE (uninstalling to get past it erases the
# wallet). The committed fingerprint turns that silent key rotation into a
# red job.
set -euo pipefail

apk="${1:?usage: verify-android-signer.sh <apk> [expected-sha256-file]}"
expected_file="${2:-}"

# apksigner is a Java program; the SDK wrapper execs `java` from PATH.
if ! command -v java >/dev/null 2>&1 && [ -n "${JAVA_HOME:-}" ]; then
  export PATH="$JAVA_HOME/bin:$PATH"
fi

find_apksigner() {
  if command -v apksigner >/dev/null 2>&1; then
    command -v apksigner
    return
  fi
  local sdk found
  for sdk in "${ANDROID_HOME:-}" "${ANDROID_SDK_ROOT:-}" "$HOME/Library/Android/sdk" "$HOME/Android/Sdk"; do
    [ -n "$sdk" ] && [ -d "$sdk/build-tools" ] || continue
    # The newest build-tools wins; plain ls would put 34.0.0 after 36.1.0.
    found=$(ls -d "$sdk"/build-tools/*/apksigner 2>/dev/null | sort -V | tail -1)
    if [ -n "$found" ]; then
      echo "$found"
      return
    fi
  done
  echo "::error::apksigner not found (ANDROID_HOME=${ANDROID_HOME:-unset}, ANDROID_SDK_ROOT=${ANDROID_SDK_ROOT:-unset})" >&2
  exit 1
}

[ -f "$apk" ] || { echo "::error::$apk does not exist" >&2; exit 1; }
apksigner=$(find_apksigner)

if ! report=$("$apksigner" verify --print-certs "$apk" 2>&1); then
  echo "$report"
  echo "::error::$(basename "$apk") is not a validly signed APK; AGP names an unsigned release output *-unsigned.apk, and nothing unsigned may ship" >&2
  exit 1
fi
echo "$report" | grep -E "^Signer #[0-9]+ certificate (DN|SHA-256 digest):"
signers=$(echo "$report" | grep -c "^Signer #[0-9]* certificate SHA-256 digest:" || true)
actual=$(echo "$report" | sed -n 's/^Signer #1 certificate SHA-256 digest: //p' | tr -d '[:space:]')
if [ -z "$actual" ]; then
  echo "::error::apksigner printed no signer certificate for $(basename "$apk")" >&2
  exit 1
fi

if [ -z "$expected_file" ]; then
  echo "signed: $(basename "$apk") ($signers signer)"
  exit 0
fi

expected=$(tr -d '[:space:]' < "$expected_file")
if ! [[ "$expected" =~ ^[0-9a-f]{64}$ ]]; then
  echo "::error::$expected_file must hold one 64-char lowercase hex SHA-256 (the signer certificate digest apksigner prints)" >&2
  exit 1
fi
if [ "$signers" -ne 1 ] || [ "$actual" != "$expected" ]; then
  echo "::error::$(basename "$apk") is signed by $actual ($signers signer), expected $expected from $expected_file. This is not the key the published betas carry: installs would fail with INSTALL_FAILED_UPDATE_INCOMPATIBLE. Restore the runner's ~/.android/debug.keystore; do not ship this APK." >&2
  exit 1
fi
echo "signer OK: $(basename "$apk") is signed by $expected"
