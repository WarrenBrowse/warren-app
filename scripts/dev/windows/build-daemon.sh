#!/usr/bin/env bash
#
# Build the Warren daemon + CLI in debug mode for local Windows development,
# and stage the runtime native libs (wintun + winfw) and resources next to the
# binary so it can run straight from target/debug.
#
# Run from a normal (non-elevated) Git Bash. The MSVC toolchain is set up by
# scripts/vcvars.sh (cl/link for the host arch). For the full app + installer,
# use build-app.sh instead.
set -eu

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$REPO"

# host target triple ($HOST) and the C++ build target (ARM64 / x64).
source scripts/utils/host
case "$HOST" in
    aarch64-pc-windows-msvc) CPP_TARGET=ARM64 ;;
    x86_64-pc-windows-msvc)  CPP_TARGET=x64 ;;
    *) echo "build-daemon.sh: unsupported Windows host '$HOST'" >&2; exit 1 ;;
esac

# MSVC env (cl/link). vcvars.sh locates vcvarsall via vswhere.
. ./scripts/vcvars.sh

# api-override matches what build.sh enables for dev builds (lets the daemon
# target a custom API at runtime).
cargo build --features api-override \
    -p mullvad-daemon --bin warren-daemon \
    -p mullvad-cli --bin warren \
    -p mullvad-setup --bin warren-setup \
    -p mullvad-problem-report --bin warren-problem-report

# Runtime native libs the daemon loads (wintun adapter + WFP firewall layer).
# winfw.dll must have been built once via ./build-windows-modules.sh (or build-app.sh).
cp -f "dist-assets/binaries/$HOST/wintun/wintun.dll" target/debug/
winfw="windows/winfw/bin/${CPP_TARGET}-Debug/winfw.dll"
if [ -f "$winfw" ]; then
    cp -f "$winfw" target/debug/
else
    echo "WARNING: $winfw not found; run ./build-windows-modules.sh first (CPP_BUILD_TARGETS=$CPP_TARGET)." >&2
fi

# Make ./dist-assets a complete WARREN_RESOURCE_DIR: build.sh writes these into
# build/, the daemon expects them in its resource dir.
[ -f build/relays.json ] && cp -f build/relays.json dist-assets/ || true
[ -f build/warren-relays.json ] && cp -f build/warren-relays.json dist-assets/ || true

echo
echo "OK. Daemon at target/debug/warren-daemon.exe"
echo "Next: scripts/dev/windows/dev-service.ps1 -Action Install (once), then -Action Start."
