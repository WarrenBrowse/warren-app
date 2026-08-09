#!/usr/bin/env bash
#
# Assert that a built winfw.dll carries the WFP object keys of the product
# environment it is about to ship with, and none of another environment's.
#
# The provider GUIDs are constexpr-salted (windows/winfw/src/winfw/mullvadguids.h),
# so the salted bytes are baked into the dll's data section at compile time and
# can be read back off the artifact. This is the last line of defense against
# every stale-reuse vector at once (CI cache restore, MSBuild up-to-date checks,
# PCH macro reuse, leftover msbuild node environments): whatever produced the
# bytes, the shipped artifact either carries the right keys or the build fails.
#
# A beta dll carrying production keys walled a user's machine for two hours on
# 2026-08-09: the old daemon's update shutdown wrote its persistent kill-switch
# under beta keys, and the mis-salted new daemon could neither see nor remove
# them (incidents/2026-08-09-* in the workspace).
#
# Usage: ci/verify-winfw-salt.sh <winfw.dll> <prod|beta|staging>

set -euo pipefail

DLL="${1:?usage: verify-winfw-salt.sh <winfw.dll> <prod|beta|staging>}"
ENV="${2:?usage: verify-winfw-salt.sh <winfw.dll> <prod|beta|staging>}"

# Keep in sync with warren_fw_guid_salt in build-windows-modules.sh.
salt_for() {
    case "$1" in
        prod)    echo 0x0 ;;
        beta)    echo 0x5BE7A001 ;;
        staging) echo 0x57A61009 ;;
        *) echo "unknown environment '$1'" >&2; exit 2 ;;
    esac
}

# MullvadGuids::ProviderPersistent() base GUID {2bc5bc63-80b0-4119-86d3-6afe0dff2a26}.
# Data1 is XORed with the environment salt and stored little-endian; the rest of
# the GUID is salt-independent and anchors the match.
BASE_DATA1=0x2bc5bc63
GUID_TAIL="b080194186d36afe0dff2a26"

pattern_for() {
    local data1 le
    data1=$(( BASE_DATA1 ^ $(salt_for "$1") ))
    le=$(printf '%02x%02x%02x%02x' \
        $((data1 & 0xff)) $(((data1 >> 8) & 0xff)) \
        $(((data1 >> 16) & 0xff)) $(((data1 >> 24) & 0xff)))
    echo "${le}${GUID_TAIL}"
}

hex=$(od -A n -t x1 -v "$DLL" | tr -d ' \n')

fail=0
for env in prod beta staging; do
    pat=$(pattern_for "$env")
    if [[ "$hex" == *"$pat"* ]]; then found=yes; else found=no; fi
    if [[ "$env" == "$ENV" && "$found" == no ]]; then
        echo "error: $DLL does not carry the $ENV WFP provider key (expected GUID bytes $pat)" >&2
        fail=1
    elif [[ "$env" != "$ENV" && "$found" == yes ]]; then
        echo "error: $DLL carries the $env WFP provider key ($pat) but this is a $ENV build" >&2
        fail=1
    fi
done

if [[ "$fail" != 0 ]]; then
    echo "error: winfw.dll was compiled with the wrong WARREN_FW_GUID_SALT." >&2
    echo "       Stale intermediates survived (cache restore, PCH, up-to-date check)." >&2
    echo "       Fix: ./build-windows-modules.sh clean && rebuild with --$ENV" >&2
    exit 1
fi

echo "winfw.dll WFP provider key matches product env '$ENV'"
