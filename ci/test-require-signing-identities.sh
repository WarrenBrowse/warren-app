#!/usr/bin/env bash
# Tests for ci/require-signing-identities.sh: a production tag is refused
# while any signing identity is missing; a beta tag is not held to it.
#
#   bash ci/test-require-signing-identities.sh
set -uo pipefail

SCRIPT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/require-signing-identities.sh"

failures=0
checks=0
check() { # check <description> <expected> <actual>
	checks=$((checks + 1))
	if [ "$2" = "$3" ]; then
		printf '  ok   %s\n' "$1"
	else
		printf '  FAIL %s (expected %s, got %s)\n' "$1" "$2" "$3"
		failures=$((failures + 1))
	fi
}

all_set() {
	WARREN_CSC_LINK_MACOS=x WARREN_CSC_KEY_PASSWORD_MACOS=x \
		WARREN_NOTARIZE_KEYCHAIN_PROFILE=x \
		AZURE_TENANT_ID=x AZURE_CLIENT_ID=x AZURE_CLIENT_SECRET=x \
		TRUSTED_SIGNING_ENDPOINT=x TRUSTED_SIGNING_ACCOUNT=x TRUSTED_SIGNING_PROFILE=x \
		ANDROID_KEYSTORE_BASE64=x WARREN_UPDATE_SIGNING_KEY=x "$@"
}
status() { "$@" > /dev/null 2>&1; echo $?; }

check "a beta tag needs no signing identity" 0 \
	"$(status env -u WARREN_CSC_LINK_MACOS bash "$SCRIPT" beta)"
check "a production tag with every identity passes" 0 \
	"$(status all_set bash "$SCRIPT" prod)"
# A signed but unnotarized pkg is still refused by Gatekeeper, and the Windows
# installer is signed through the cloud signing service wired in
# ci/codemagic/windows-release.sh.
for missing in WARREN_CSC_LINK_MACOS WARREN_CSC_KEY_PASSWORD_MACOS \
	WARREN_NOTARIZE_KEYCHAIN_PROFILE \
	AZURE_TENANT_ID AZURE_CLIENT_ID AZURE_CLIENT_SECRET \
	TRUSTED_SIGNING_ENDPOINT TRUSTED_SIGNING_ACCOUNT TRUSTED_SIGNING_PROFILE \
	ANDROID_KEYSTORE_BASE64 WARREN_UPDATE_SIGNING_KEY; do
	check "a production tag without $missing is refused" 1 \
		"$(status all_set env "$missing=" bash "$SCRIPT" prod)"
done
out=$(env -u WARREN_CSC_LINK_MACOS bash "$SCRIPT" prod 2>&1)
check "the refusal names what is missing" yes \
	"$(case "$out" in *WARREN_CSC_LINK_MACOS*) echo yes ;; *) echo no ;; esac)"
check "an unknown channel is refused" 1 "$(status all_set bash "$SCRIPT" staging)"

echo "$checks checks, $failures failed"
[ "$failures" -eq 0 ]
