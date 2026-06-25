# shellcheck shell=bash
#
# Sourcing this file should set up the appropriate environment for Visual Studio using vcvarsall.bat
#
# Currently, this script runs vcvarsall.bat and exports the following (after appropriate
# conversions):
# * PATH
# * INCLUDE

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"

# shellcheck source=/dev/null
source "$SCRIPT_DIR/utils/host"

case $HOST in
    x86_64-pc-windows-msvc) HOST_TARGET=x64;;
    aarch64-pc-windows-msvc) HOST_TARGET=arm64;;
    *)
        log_error "Unexpected architecture: $HOST"
        exit 1
        ;;
esac

# Target architecture. Use the host architecture if unspecified.
TARGET=${TARGET:-"$HOST_TARGET"}

# Locate vcvarsall.bat. Prefer vswhere so any VS 2022 edition (Community,
# Professional, Enterprise, Build Tools) at any install path is found, including
# Build Tools under "Program Files (x86)" on ARM64 hosts; fall back to the
# well-known fixed locations if vswhere is unavailable.
VCVARSPATH=""
VSWHERE="C:\\Program Files (x86)\\Microsoft Visual Studio\\Installer\\vswhere.exe"
if [[ -f "$VSWHERE" ]]; then
    vs_install="$("$VSWHERE" -latest -products '*' -property installationPath 2>/dev/null | tr -d '\r')"
    if [[ -n "$vs_install" && -f "$vs_install\\VC\\Auxiliary\\Build\\vcvarsall.bat" ]]; then
        VCVARSPATH="$vs_install\\VC\\Auxiliary\\Build\\vcvarsall.bat"
    fi
fi
if [[ -z "$VCVARSPATH" ]]; then
    for cand in \
        "C:\\Program Files\\Microsoft Visual Studio\\2022\\Community\\VC\\Auxiliary\\Build\\vcvarsall.bat" \
        "C:\\Program Files\\Microsoft Visual Studio\\2022\\BuildTools\\VC\\Auxiliary\\Build\\vcvarsall.bat" \
        "C:\\Program Files (x86)\\Microsoft Visual Studio\\2022\\BuildTools\\VC\\Auxiliary\\Build\\vcvarsall.bat"; do
        if [[ -f "$cand" ]]; then
            VCVARSPATH="$cand"
            break
        fi
    done
fi

if [[ -z "$VCVARSPATH" || ! -f "$VCVARSPATH" ]]; then
    echo -e "vcvarsall.bat not found. Install VS 2022 (Community or Build Tools), or update ${BASH_SOURCE[0]}"
    exit 1
fi

VCVARSENV=$(MSYS_NO_PATHCONV=1 MSYS2_ARG_CONV_EXCL='*' cmd.exe /c "$VCVARSPATH" "$TARGET" \>nul \& set)

declare -A vcenvmap

function populate_vcenvmap {
    while IFS='=' read -r key value; do
        vcenvmap[$key]=$value
    done <<< "$VCVARSENV"
}

function to_unix_path {
    # Converts a Windows-style PATH to a UNIX-style PATH
    # eg from "C:\1\2\3;C:\4\5\6" to "/c/1/2/3:/c/4/5/6"
    echo "$1" | sed -e 's|\([a-zA-Z]\):|\/\1|g' -e 's|\\|/|g' -e 's|;|:|g'
}

populate_vcenvmap

export INCLUDE="${vcenvmap["INCLUDE"]}"
PATH="$(to_unix_path "${vcenvmap["PATH"]}")"
export PATH

echo "Initialized VS environment for $TARGET"
