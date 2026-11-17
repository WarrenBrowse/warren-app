#!/usr/bin/env bash
# Refuse a Windows binary the release would ship without the loader's
# exploit mitigations: ASLR, high-entropy ASLR (64-bit images), DEP and Control
# Flow Guard. The daemon and winfw run as SYSTEM and parse network input and
# IPC, so a build flag that quietly drops out has to fail the release.
#
#   ci/check-pe-hardening.sh <image.exe|image.dll>...
#
# Reads the DllCharacteristics field of the optional header, which sits at the
# same offset in PE32 and PE32+ images.
set -euo pipefail

HIGH_ENTROPY_VA=0x0020
DYNAMIC_BASE=0x0040
NX_COMPAT=0x0100
GUARD_CF=0x4000

# le <file> <offset> <width>: one little-endian field.
le() {
    local value=0 shift=0 byte
    for byte in $(od -An -tu1 -j "$2" -N "$3" "$1"); do
        value=$((value | (byte << shift)))
        shift=$((shift + 8))
    done
    [ "$shift" -eq $((8 * $3)) ] || return 1
    echo "$value"
}

[ "$#" -gt 0 ] || { echo "usage: check-pe-hardening.sh <image>..." >&2; exit 2; }

status=0
for image in "$@"; do
    if ! pe="$(le "$image" 60 4)" \
        || [ "$(od -An -c -j "$pe" -N2 "$image" | tr -d ' \n')" != PE ] \
        || ! magic="$(le "$image" $((pe + 24)) 2)" \
        || ! flags="$(le "$image" $((pe + 24 + 70)) 2)"; then
        echo "::error::$image is not a PE image" >&2
        status=1
        continue
    fi
    missing=()
    [ $((flags & DYNAMIC_BASE)) -ne 0 ] || missing+=("ASLR")
    [ $((flags & NX_COMPAT)) -ne 0 ] || missing+=("DEP")
    [ $((flags & GUARD_CF)) -ne 0 ] || missing+=("Control Flow Guard")
    # 0x20b is PE32+: only a 64-bit address space has the entropy to ask for.
    if [ "$magic" -eq $((0x20b)) ] && [ $((flags & HIGH_ENTROPY_VA)) -eq 0 ]; then
        missing+=("high-entropy ASLR")
    fi
    if [ "${#missing[@]}" -gt 0 ]; then
        echo "::error::$image is built without: ${missing[*]}" >&2
        status=1
    else
        echo "$image: ASLR, DEP and Control Flow Guard on"
    fi
done
exit "$status"
