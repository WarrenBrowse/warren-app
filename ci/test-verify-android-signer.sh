#!/usr/bin/env bash
# Runs ci/verify-android-signer.sh against a stand-in apksigner printing each
# output format the build-tools have used, so a new format fails here and not
# in the release job.
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
checks=0
failures=0
ok() { checks=$((checks + 1)); echo "  ok   $1"; }
fail() { checks=$((checks + 1)); failures=$((failures + 1)); echo "  FAIL $1"; }

KEY=2d8b79941e00f195e5cacfe3feb9f54904548536b8fe1a0f1aaea0ec7ddc8064
OTHER=0000000000000000000000000000000000000000000000000000000000000000
printf '%s\n' "$KEY" > "$TMP/expected"
touch "$TMP/app.apk"
mkdir -p "$TMP/bin"

# $1: the text the stand-in apksigner prints for `verify --print-certs`.
with_report() {
	printf '%s\n' "$1" > "$TMP/report"
	cat > "$TMP/bin/apksigner" <<STUB
#!/bin/sh
cat "$TMP/report"
STUB
	chmod +x "$TMP/bin/apksigner"
}

run() { PATH="$TMP/bin:$PATH" bash "$SCRIPT_DIR/verify-android-signer.sh" "$TMP/app.apk" "$TMP/expected" > /dev/null 2>&1; }

expect() { # $1: description, $2: expected exit (0 or 1)
	run
	local rc=$?
	if { [ "$2" -eq 0 ] && [ "$rc" -eq 0 ]; } || { [ "$2" -ne 0 ] && [ "$rc" -ne 0 ]; }; then ok "$1"; else fail "$1 (exit $rc)"; fi
}

with_report "Signer #1 certificate DN: C=US, O=Android, CN=Android Debug
Signer #1 certificate SHA-256 digest: $KEY"
expect "the numbered format of older build-tools is accepted" 0

with_report "V2 Signer: certificate DN: C=US, O=Android, CN=Android Debug
V2 Signer: certificate SHA-256 digest: $KEY
V2 Signer: certificate SHA-1 digest: b5ff64600e96b71e9a385db3f7e58b63d96e3bce"
expect "the per-scheme format of build-tools 37 is accepted" 0

with_report "V2 Signer: certificate SHA-256 digest: $KEY
V3 Signer: certificate SHA-256 digest: $KEY"
expect "one key signing under two schemes counts as one signer" 0

with_report "V2 Signer: certificate SHA-256 digest: $OTHER"
expect "another key is refused" 1

with_report "Signer #1 certificate SHA-256 digest: $KEY
Signer #2 certificate SHA-256 digest: $OTHER"
expect "a second signer is refused" 1

with_report "Verifies"
expect "a report naming no certificate is refused" 1

printf '\n%d checks, %d failure(s)\n' "$checks" "$failures"
[ "$failures" -eq 0 ]
