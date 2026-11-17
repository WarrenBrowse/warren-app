#!/usr/bin/env bash
# Tests for ci/check-nsis-stub.sh, which refuses a Windows installer whose NSIS
# stub predates the releases that fixed the elevated installer's temporary
# directories.
#
#   bash ci/test-check-nsis-stub.sh
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CHECK="$SCRIPT_DIR/check-nsis-stub.sh"
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

# le32 <value>: four little-endian bytes as printf escapes.
le32() {
	printf '\\%03o\\%03o\\%03o\\%03o' $(($1 & 255)) $((($1 >> 8) & 255)) \
		$((($1 >> 16) & 255)) $((($1 >> 24) & 255))
}

# pe <path> <TimeDateStamp>: the smallest file with a DOS header pointing at a
# PE header that carries the given link time.
pe() {
	{
		printf 'MZ'
		head -c 58 /dev/zero
		# shellcheck disable=SC2059
		printf "$(le32 128)"
		head -c 64 /dev/zero
		printf 'PE\000\000\114\001\001\000'
		# shellcheck disable=SC2059
		printf "$(le32 "$2")"
		head -c 64 /dev/zero
	} > "$1"
}

# 2018-12-15T22:26:14Z, the stub of NSIS 3.04 every release shipped with.
pe "$TMP/old.exe" 1544912774
bash "$CHECK" "$TMP/old.exe" > /dev/null 2>&1
check "refuses the NSIS 3.04 stub" 1 "$?"

# 2026-04-19, the release date of NSIS 3.12.
pe "$TMP/new.exe" 1776556800
bash "$CHECK" "$TMP/new.exe" > /dev/null 2>&1
check "accepts a stub built after the temporary-directory fixes" 0 "$?"

printf 'MZ not a PE' > "$TMP/garbage.exe"
bash "$CHECK" "$TMP/garbage.exe" > /dev/null 2>&1
check "refuses a file that is not a PE image" 1 "$?"

bash "$CHECK" "$TMP/missing.exe" > /dev/null 2>&1
check "refuses a missing installer" 1 "$?"

printf '\n%d checks, %d failure(s)\n' "$checks" "$failures"
[ "$failures" -eq 0 ]
