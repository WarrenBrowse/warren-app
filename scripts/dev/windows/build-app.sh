#!/usr/bin/env bash
#
# Build the full Warren VPN app + NSIS installer on Windows (daemon + CLI +
# C++ modules + Electron app), for the host architecture.
#
# Run from a normal (non-elevated) Git Bash. Extra build.sh flags pass through,
# e.g. `build-app.sh --beta --optimize`. The installer lands in dist/.
#
# This wrapper exists because build.sh expects the VS toolchain already sourced
# AND msbuild.exe on PATH (vcvarsall does not add MSBuild), AND `TARGETS` set so
# electron-builder packages for the host arch instead of defaulting to x64.
#
# Usage: scripts/dev/windows/build-app.sh <--prod|--beta|--staging> [build.sh flags]
set -eu

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$REPO"

# shellcheck source=../../utils/product-env.sh
source scripts/utils/product-env.sh

usage() {
    cat <<EOF
Build the full Warren VPN app + NSIS installer on Windows.

Usage: $(basename "$0") <--prod|--beta|--staging> [build.sh flags]

Environment (required, no default: the API host, the update channel, the app
id, the NSIS GUIDs and the artifact names are all compiled in):
  --prod       Production build
  --beta       Beta build
  --staging    Staging build
  The WARREN_PRODUCT_ENV env var is used when no flag is given, which is what
  the wbuild.cmd / wbuildbeta.cmd helpers in the dev VM rely on.

Every other flag passes through to build.sh (--optimize, --sign, ...).
EOF
}

# The environment is settled here rather than left to build.sh, so a bare
# invocation stops on the help instead of quietly producing a prod installer.
ENV_FLAG=""
BUILD_ARGS=()
for arg in "$@"; do
    if warren_env_flag "$arg"; then
        ENV_FLAG="$WARREN_ENV_FLAG"
        continue
    fi
    case "$arg" in
        -h|--help) usage; exit 0 ;;
        *) BUILD_ARGS+=("$arg") ;;
    esac
done
warren_env_require "$ENV_FLAG"

source scripts/utils/host
case "$HOST" in
    aarch64-pc-windows-msvc) MSBUILD_ARCH=arm64 ;;
    x86_64-pc-windows-msvc)  MSBUILD_ARCH=amd64 ;;
    *) echo "build-app.sh: unsupported Windows host '$HOST'" >&2; exit 1 ;;
esac

# MSVC env (INCLUDE/LIB/PATH for the host arch). Overwrites PATH wholesale.
. ./scripts/vcvars.sh

# vcvarsall does NOT put msbuild.exe on PATH; add it after vcvars (which it would
# otherwise wipe). Locate the VS install via vswhere, same edition vcvars found.
VSWHERE="C:\\Program Files (x86)\\Microsoft Visual Studio\\Installer\\vswhere.exe"
if [ -f "$VSWHERE" ]; then
    vs_install="$("$VSWHERE" -latest -products '*' -property installationPath 2>/dev/null | tr -d '\r')"
    if [ -n "$vs_install" ]; then
        msbuild_bin="$(cygpath -u "$vs_install")/MSBuild/Current/Bin"
        export PATH="$msbuild_bin/$MSBUILD_ARCH:$msbuild_bin:$(dirname "$(cygpath -u "$VSWHERE")"):$PATH"
    fi
fi
command -v msbuild >/dev/null 2>&1 || command -v MSBuild.exe >/dev/null 2>&1 || {
    echo "build-app.sh: msbuild not found on PATH after vcvars; cannot build the C++ modules." >&2
    exit 1
}

# Package for the host arch. Without TARGETS, electron-builder's pack-windows
# defaults to x64 (it only switches to arm64 when --targets=aarch64-pc-windows-msvc
# is passed, which build.sh only forwards when TARGETS is set).
export TARGETS="$HOST"

exec ./build.sh ${BUILD_ARGS[@]+"${BUILD_ARGS[@]}"}
