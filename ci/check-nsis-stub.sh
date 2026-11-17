#!/usr/bin/env bash
# Refuse a Windows installer assembled on an NSIS stub older than 3.11.
#
#   ci/check-nsis-stub.sh <installer.exe>...
#
# The installer runs elevated and launches warren-setup.exe out of its plugin
# directory. NSIS 3.09, 3.11 and 3.12 each hardened that temporary directory
# against another local process, and electron-builder silently falls back to
# its legacy NSIS 3.04 toolset when the `toolsets.nsis` pin in distribution.cjs
# is lost. The link time in the PE header is the stub's build time, so it tells
# the two apart without running the installer: the 3.04 stub reads 2018-12-15.
set -euo pipefail

# 2025-03-08T00:00:00Z, the release of NSIS 3.11.
FLOOR=1741392000

# u32 <file> <offset>: one little-endian 32-bit field.
u32() {
    local bytes
    bytes="$(od -An -tu1 -j "$2" -N4 "$1" | tr -s ' \n' ' ')"
    read -r b0 b1 b2 b3 <<< "$bytes"
    [ -n "${b3:-}" ] || return 1
    echo $((b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)))
}

[ "$#" -gt 0 ] || { echo "usage: check-nsis-stub.sh <installer.exe>..." >&2; exit 2; }

for installer in "$@"; do
    [ -f "$installer" ] || { echo "::error::$installer does not exist" >&2; exit 1; }
    pe_offset="$(u32 "$installer" 60)" \
        && [ "$(od -An -c -j "$pe_offset" -N2 "$installer" | tr -d ' \n')" = PE ] \
        && stamp="$(u32 "$installer" $((pe_offset + 8)))" \
        || { echo "::error::$installer is not a PE image" >&2; exit 1; }
    if [ "$stamp" -lt "$FLOOR" ]; then
        echo "::error::$installer was assembled on an NSIS stub linked at $stamp (before NSIS 3.11); check toolsets.nsis in distribution.cjs" >&2
        exit 1
    fi
    echo "$installer: NSIS stub linked at $stamp"
done
