#!/usr/bin/env bash
# Reproducible smoke for the warren-jni native side - meant to be runnable
# from a clean checkout on macOS or Linux without Android Studio. Verifies
# the warren-jni Android delivery surface:
#
#   1. The `warren-jni` crate compiles for the three Android ABIs we ship
#      (aarch64-linux-android, armv7-linux-androideabi, x86_64-linux-android).
#   2. The host-runnable wallet unit tests pass (BIP39 generation, Ed25519
#      derivation, canonical X-Warren-* request signing).
#   3. `cargo clippy -D warnings` is clean on host + aarch64-linux-android.
#
# Pre-conditions (one-time setup):
#   - Android SDK + NDK r26 installed at $ANDROID_HOME / $ANDROID_NDK_HOME
#     (defaults to ~/Library/Android/sdk and ~/Library/Android/sdk/ndk/26.x).
#   - `rustup target add aarch64-linux-android armv7-linux-androideabi
#      x86_64-linux-android` already run.
#   - `android/local.properties` seeded (gitignored, created by Android Studio
#     or by hand with `sdk.dir=` + `ndk.dir=`).
#
# Exit code: 0 on full PASS, non-zero on any failure (cargo bubbles its own
# exit code up).

set -euo pipefail

# Resolve repo root from this script's location.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

# Defaults if the caller did not export the toolchain envs already.
: "${ANDROID_HOME:=$HOME/Library/Android/sdk}"
# Derived from the gradle version catalogue rather than spelled out: a second
# copy drifts, and this one already had, defaulting to an NDK the build does
# not pin and no machine here installs.
NDK_PINNED="$(sed -n 's/^ndk = "\(.*\)"$/\1/p' "$REPO_ROOT/android/gradle/libs.versions.toml")"
if [[ -z "$NDK_PINNED" ]]; then
    echo "FAIL: no 'ndk = \"...\"' pin found in android/gradle/libs.versions.toml." >&2
    exit 1
fi
: "${ANDROID_NDK_HOME:=$ANDROID_HOME/ndk/$NDK_PINNED}"

if [[ ! -d "$ANDROID_NDK_HOME" ]]; then
    echo "FAIL: ANDROID_NDK_HOME=$ANDROID_NDK_HOME does not exist." >&2
    echo "      Set ANDROID_NDK_HOME to your NDK install path." >&2
    exit 1
fi

NDK_BIN="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/darwin-x86_64/bin"
if [[ ! -d "$NDK_BIN" ]]; then
    # Fall back to linux-x86_64 prebuilts when not on macOS.
    NDK_BIN="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin"
fi

export CC_aarch64_linux_android="$NDK_BIN/aarch64-linux-android26-clang"
export CC_armv7_linux_androideabi="$NDK_BIN/armv7a-linux-androideabi26-clang"
export CC_x86_64_linux_android="$NDK_BIN/x86_64-linux-android26-clang"
export AR_aarch64_linux_android="$NDK_BIN/llvm-ar"
export AR_armv7_linux_androideabi="$NDK_BIN/llvm-ar"
export AR_x86_64_linux_android="$NDK_BIN/llvm-ar"
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$CC_aarch64_linux_android"
export CARGO_TARGET_ARMV7_LINUX_ANDROIDEABI_LINKER="$CC_armv7_linux_androideabi"
export CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER="$CC_x86_64_linux_android"

cd "$REPO_ROOT"

echo "==> warren-jni host wallet tests"
cargo test -p warren-jni --lib --quiet

echo "==> warren-jni host clippy"
cargo clippy -p warren-jni --all-targets --quiet -- -D warnings

echo "==> warren-jni cargo check for 3 Android ABIs"
for target in aarch64-linux-android armv7-linux-androideabi x86_64-linux-android; do
    echo "    - $target"
    cargo check -p warren-jni --target "$target" --quiet
done

echo "==> warren-jni clippy on aarch64-linux-android"
cargo clippy -p warren-jni --target aarch64-linux-android --quiet -- -D warnings

echo ""
echo "All warren-jni delivery checks PASSED."
