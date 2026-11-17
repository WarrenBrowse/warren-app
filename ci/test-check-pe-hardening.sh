#!/usr/bin/env bash
# Tests for ci/check-pe-hardening.sh, which refuses a shipped Windows binary
# built without ASLR, high-entropy ASLR, DEP or Control Flow Guard.
#
#   bash ci/test-check-pe-hardening.sh
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CHECK="$SCRIPT_DIR/check-pe-hardening.sh"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

failures=0
checks=0
check() { # check <description> <expected exit code> <actual exit code>
	checks=$((checks + 1))
	if [ "$2" = "$3" ]; then
		printf '  ok   %s\n' "$1"
	else
		printf '  FAIL %s (expected exit %s, got %s)\n' "$1" "$2" "$3"
		failures=$((failures + 1))
	fi
}

# le <width> <value>: little-endian bytes as printf escapes.
le() {
	local i out=""
	for ((i = 0; i < $1; i++)); do
		out+="$(printf '\\%03o' $((($2 >> (8 * i)) & 255)))"
	done
	printf '%s' "$out"
}

# pe <path> <optional header magic> <DllCharacteristics>: a DOS header, the PE
# signature, a COFF header and the start of an optional header, which is all
# the check reads.
pe() {
	{
		printf 'MZ'
		head -c 58 /dev/zero
		# shellcheck disable=SC2059
		printf "$(le 4 64)"
		printf 'PE\000\000'
		head -c 20 /dev/zero
		# shellcheck disable=SC2059
		printf "$(le 2 "$2")"
		head -c 68 /dev/zero
		# shellcheck disable=SC2059
		printf "$(le 2 "$3")"
		head -c 64 /dev/zero
	} > "$1"
}

# HIGH_ENTROPY_VA | DYNAMIC_BASE | NX_COMPAT | GUARD_CF
hardened=$((0x0020 | 0x0040 | 0x0100 | 0x4000))

pe "$TMP/ok.exe" $((0x20b)) "$hardened"
bash "$CHECK" "$TMP/ok.exe" > /dev/null 2>&1
check "accepts a 64-bit image with ASLR, high-entropy ASLR, DEP and CFG" 0 "$?"

pe "$TMP/nocfg.exe" $((0x20b)) $((hardened & ~0x4000))
bash "$CHECK" "$TMP/nocfg.exe" > /dev/null 2>&1
check "refuses an image built without Control Flow Guard" 1 "$?"

pe "$TMP/nonx.dll" $((0x20b)) $((hardened & ~0x0100))
bash "$CHECK" "$TMP/nonx.dll" > /dev/null 2>&1
check "refuses an image without DEP" 1 "$?"

pe "$TMP/noaslr.exe" $((0x20b)) $((hardened & ~0x0040))
bash "$CHECK" "$TMP/noaslr.exe" > /dev/null 2>&1
check "refuses an image without ASLR" 1 "$?"

pe "$TMP/lowentropy.exe" $((0x20b)) $((hardened & ~0x0020))
bash "$CHECK" "$TMP/lowentropy.exe" > /dev/null 2>&1
check "refuses a 64-bit image without high-entropy ASLR" 1 "$?"

pe "$TMP/x86.dll" $((0x10b)) $((hardened & ~0x0020))
bash "$CHECK" "$TMP/x86.dll" > /dev/null 2>&1
check "does not ask a 32-bit image for high-entropy ASLR" 0 "$?"

bash "$CHECK" "$TMP/ok.exe" "$TMP/nocfg.exe" > /dev/null 2>&1
check "refuses a set with one weak image in it" 1 "$?"

printf 'MZ' > "$TMP/garbage.exe"
bash "$CHECK" "$TMP/garbage.exe" > /dev/null 2>&1
check "refuses a file that is not a PE image" 1 "$?"

printf '\n%d checks, %d failure(s)\n' "$checks" "$failures"
[ "$failures" -eq 0 ]
