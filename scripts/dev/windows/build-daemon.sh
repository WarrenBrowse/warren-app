#!/usr/bin/env bash
#
# Build the Warren daemon + CLI in debug mode for local Windows development,
# and stage the runtime native libs (wintun + winfw) and resources next to the
# binary so it can run straight from target/debug.
#
# Run from a normal (non-elevated) Git Bash. The MSVC toolchain is set up by
# scripts/vcvars.sh (cl/link for the host arch). For the full app + installer,
# use build-app.sh instead.
#
# Usage: scripts/dev/windows/build-daemon.sh <--prod|--beta|--staging>
set -eu

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$REPO"

# shellcheck source=../../utils/product-env.sh
source scripts/utils/product-env.sh

usage() {
    cat <<EOF
Build the Warren daemon + CLI for local Windows development.

Usage: $(basename "$0") <--prod|--beta|--staging>

Environment (required, no default: the API host, the update channel, the pipe
name and the WFP object keys are all compiled in):
  --prod       Production build
  --beta       Beta build
  --staging    Staging build
  The WARREN_PRODUCT_ENV env var is used when no flag is given, which is what
  the wbuild.cmd / wbuildbeta.cmd helpers in the dev VM rely on.

Options:
  -h, --help   Show this help
EOF
}

ENV_FLAG=""
for arg in "$@"; do
    if warren_env_flag "$arg"; then
        ENV_FLAG="$WARREN_ENV_FLAG"
        continue
    fi
    case "$arg" in
        -h|--help) usage; exit 0 ;;
        *)
            usage >&2
            printf '\nerror: unknown argument: %s\n' "$arg" >&2
            exit 1
            ;;
    esac
done
warren_env_require "$ENV_FLAG"

# host target triple ($HOST) and the C++ build target (ARM64 / x64).
source scripts/utils/host
case "$HOST" in
    aarch64-pc-windows-msvc) CPP_TARGET=ARM64 ;;
    x86_64-pc-windows-msvc)  CPP_TARGET=x64 ;;
    *) echo "build-daemon.sh: unsupported Windows host '$HOST'" >&2; exit 1 ;;
esac

# MSVC env (cl/link). vcvars.sh locates vcvarsall via vswhere.
. ./scripts/vcvars.sh

echo "Building the daemon for the $WARREN_PRODUCT_ENV environment..."
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
winfw_dir="windows/winfw/bin/${CPP_TARGET}-Debug"
winfw="$winfw_dir/winfw.dll"
if [ -f "$winfw" ]; then
    # winfw.dll derives its WFP object keys from a per-environment salt baked
    # in at compile time. Pairing the daemon with a dll salted for another
    # environment arms the kill switch under object keys this environment's
    # teardown never sweeps, and the machine stays blocked with nothing able
    # to find the filters. Older dlls carry no stamp, hence the soft case.
    winfw_env="$(cat "$winfw_dir/.warren-product-env" 2>/dev/null || true)"
    if [ -z "$winfw_env" ]; then
        echo "WARNING: $winfw predates the environment stamp; rebuild it with" >&2
        echo "         ./build-windows-modules.sh --$WARREN_PRODUCT_ENV winfw if it was not" >&2
        echo "         built for $WARREN_PRODUCT_ENV." >&2
    elif [ "$winfw_env" != "$WARREN_PRODUCT_ENV" ]; then
        echo "error: $winfw was built for '$winfw_env', this daemon is '$WARREN_PRODUCT_ENV'." >&2
        echo "Rebuild it: ./build-windows-modules.sh --$WARREN_PRODUCT_ENV winfw" >&2
        exit 1
    fi
    cp -f "$winfw" target/debug/
else
    echo "WARNING: $winfw not found; run ./build-windows-modules.sh --$WARREN_PRODUCT_ENV first (CPP_BUILD_TARGETS=$CPP_TARGET)." >&2
fi

# Make ./dist-assets a complete WARREN_RESOURCE_DIR: build.sh writes these into
# build/, the daemon expects them in its resource dir.
[ -f build/relays.json ] && cp -f build/relays.json dist-assets/ || true
[ -f build/warren-relays.json ] && cp -f build/warren-relays.json dist-assets/ || true

echo
echo "OK. Daemon at target/debug/warren-daemon.exe (environment: $WARREN_PRODUCT_ENV)"
echo "Next: scripts/dev/windows/dev-service.ps1 -Action Install (once), then -Action Start."
