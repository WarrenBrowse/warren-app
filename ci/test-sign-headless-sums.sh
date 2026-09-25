#!/usr/bin/env bash
# Tests for ci/sign-headless-sums.sh, the signer of warren-cli's SHA256SUMS.
#
#   bash ci/test-sign-headless-sums.sh
#
# The installers in warren-cli refuse any release whose checksum list does not
# carry this signer's output, so what they rely on is checked here with
# ssh-keygen, and the exact bytes for a fixed test key are pinned:
# warren-cli's scripts/testdata/signed-sums holds those same bytes and its
# scripts/test-install.sh verifies them with both of the installers' paths
# (ssh-keygen, and OpenSSL 3, which this runner's 1.1.1 cannot do). That pair
# of tests is what ties the two repositories to one format.
# Needs OpenSSL 1.1.1+ and ssh-keygen from OpenSSH 8.1+.
set -uo pipefail
export LC_ALL=C

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WARREN_SIGN_LIB=1
export WARREN_SIGN_LIB
# shellcheck source=./sign-headless-sums.sh
. "$SCRIPT_DIR/sign-headless-sums.sh"
set +e

failures=0
checks=0
ok() {
	checks=$((checks + 1))
	printf '  ok   %s\n' "$1"
}
fail() {
	checks=$((checks + 1))
	failures=$((failures + 1))
	printf '  FAIL %s\n' "$1"
}
expect() { # expect <description> <command...>
	local description="$1"
	shift
	if "$@" > /dev/null 2>&1; then ok "$description"; else fail "$description"; fi
}
refuse() { # refuse <description> <command...>
	local description="$1"
	shift
	if "$@" > /dev/null 2>&1; then fail "$description (it succeeded)"; else ok "$description"; fi
}

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

# A fixed throwaway key, public knowledge on purpose: it exists to pin bytes.
TEST_SEED=7777777777777777777777777777777777777777777777777777777777777777
TEST_PUB=c853ad0f0cd2b619aea92ceec4fd56a24d6499d584ce79257e45cfd8139b60a7

seed_pem() { # seed_pem <seed> <out.pem>
	hex_to_file "302e020100300506032b657004220420$1" "$TMP/seed.der"
	openssl pkey -inform DER -in "$TMP/seed.der" -out "$2"
}
seed_pem "$TEST_SEED" "$TMP/test.pem"
if [ "$(public_key_hex "$TMP/test.pem")" != "$TEST_PUB" ]; then
	echo "  FAIL the test key does not derive to its pinned public key" >&2
	exit 1
fi
printf '# trusted for this test only\n%s\n' "$TEST_PUB" > "$TMP/trusted"

list() { # list <dir>: a checksum list as the finalise job writes it
	rm -rf "$1"
	mkdir -p "$1"
	printf '%s  %s\n' \
		0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef warren-vpn-daemon-beta_1.2.3_amd64.deb \
		fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210 warren-headless-beta-1.2.3-windows-x64.zip \
		> "$1/SHA256SUMS"
}
sshsig_verifies() { # sshsig_verifies <dir> [namespace]
	printf 'warren-release ssh-ed25519 %s\n' "$( {
		printf '\x00\x00\x00\x0bssh-ed25519\x00\x00\x00\x20'
		hex_to_file "$TEST_PUB" /dev/stdout
	} | openssl base64 -A)" > "$TMP/signers"
	ssh-keygen -Y verify -f "$TMP/signers" -I warren-release -n "${2:-warren-cli-sha256sums/1}" \
		-s "$1/SHA256SUMS.sshsig" < "$1/SHA256SUMS"
}
sha256() { openssl dgst -sha256 -r "$1" | cut -d' ' -f1; }

echo "signing"
D="$TMP/release"
list "$D"
expect "a list is signed" sign_sums "$D/SHA256SUMS" "$TEST_SEED" "$TMP/trusted"
expect "by the key, in namespace warren-cli-sha256sums/1" sshsig_verifies "$D"
refuse "and in no other namespace" sshsig_verifies "$D" file
# warren-cli's scripts/testdata/signed-sums/SHA256SUMS.sshsig is this file.
[ "$(sha256 "$D/SHA256SUMS.sshsig")" = b33ccdeeb742cff1b6c0c2aa6b98a10cbb0b43ad65028d0b3b721628bb9933b8 ] \
	&& ok "byte for byte the pinned vector" \
	|| fail "byte for byte the pinned vector (got $(sha256 "$D/SHA256SUMS.sshsig"))"
[ ! -e "$D/SHA256SUMS.sig" ] && ok "and nothing else is written" || fail "and nothing else is written"

printf 'b' >> "$D/SHA256SUMS"
refuse "a list changed after signing no longer verifies" sshsig_verifies "$D"

echo "the desktop list"
# The desktop app's list gets its own namespace, so a desktop signature can
# never stand in for a CLI one, nor the reverse.
list "$D"
expect "a list is signed in the namespace it is given" \
	env WARREN_SUMS_NAMESPACE=warren-desktop-sha256sums/1 \
	bash -c ". '$SCRIPT_DIR/sign-headless-sums.sh'; sign_sums '$D/SHA256SUMS' $TEST_SEED '$TMP/trusted'"
expect "and verifies there" sshsig_verifies "$D" warren-desktop-sha256sums/1
refuse "but not as a CLI list" sshsig_verifies "$D"

echo "refusals"
list "$D"
OTHER_SEED=8888888888888888888888888888888888888888888888888888888888888888
refuse "a key the trusted list does not name signs nothing" \
	sign_sums "$D/SHA256SUMS" "$OTHER_SEED" "$TMP/trusted"
[ ! -e "$D/SHA256SUMS.sshsig" ] && ok "and leaves no signature behind" \
	|| fail "and leaves no signature behind"
bad_seed="$(sign_sums "$D/SHA256SUMS" "${TEST_SEED}00" "$TMP/trusted" 2>&1)" \
	&& fail "a key that is not exactly 64 hex digits is refused (it succeeded)" \
	|| case "$bad_seed" in
		*"64-hex Ed25519 seed"*) ok "a key that is not exactly 64 hex digits is refused as such" ;;
		*) fail "a key that is not exactly 64 hex digits is refused as such (said: $bad_seed)" ;;
	esac
refuse "so is a missing list" sign_sums "$TMP/nowhere/SHA256SUMS" "$TEST_SEED" "$TMP/trusted"
no_key="$(env -u WARREN_UPDATE_SIGNING_KEY -u WARREN_SIGN_LIB \
	bash "$SCRIPT_DIR/sign-headless-sums.sh" "$D/SHA256SUMS" 2>&1)" \
	&& fail "a release job without the key refuses rather than publishing unsigned (it succeeded)" \
	|| case "$no_key" in
		*"WARREN_UPDATE_SIGNING_KEY is not set"*) ok "a release job without the key refuses rather than publishing unsigned" ;;
		*) fail "a release job without the key refuses rather than publishing unsigned (said: $no_key)" ;;
	esac
# An ssh-keygen whose signature does not verify: the signer checks its own
# output and publishes nothing.
mkdir "$TMP/bad-signer"
cat > "$TMP/bad-signer/ssh-keygen" << EOF
#!/bin/sh
case "\$*" in *"-Y sign"*) for last; do :; done; echo corrupted > "\$last.sig"; exit 0 ;; esac
exec "$(command -v ssh-keygen)" "\$@"
EOF
chmod +x "$TMP/bad-signer/ssh-keygen"
list "$D"
refuse "a signature that does not verify is never written" \
	env PATH="$TMP/bad-signer:$PATH" bash -c ". '$SCRIPT_DIR/sign-headless-sums.sh'; sign_sums '$D/SHA256SUMS' $TEST_SEED '$TMP/trusted'"
[ ! -e "$D/SHA256SUMS.sshsig" ] && ok "and nothing is left behind" || fail "and nothing is left behind"
# The tools the signer runs record the environment they were given.
mkdir "$TMP/spy"
for tool in openssl ssh-keygen; do
	cat > "$TMP/spy/$tool" << EOF
#!/bin/sh
env >> "$TMP/spy/seen"
exec "$(command -v "$tool")" "\$@"
EOF
	chmod +x "$TMP/spy/$tool"
done
list "$D"
env -u WARREN_SIGN_LIB PATH="$TMP/spy:$PATH" WARREN_UPDATE_SIGNING_KEY="$TEST_SEED" \
	bash "$SCRIPT_DIR/sign-headless-sums.sh" "$D/SHA256SUMS" > /dev/null 2>&1
if [ -s "$TMP/spy/seen" ] && ! grep -q "$TEST_SEED" "$TMP/spy/seen"; then
	ok "no tool the signer runs inherits the key in its environment"
else
	fail "no tool the signer runs inherits the key in its environment"
fi
refuse "the release key file refuses the test key" \
	sign_sums "$D/SHA256SUMS" "$TEST_SEED" "$SCRIPT_DIR/../mullvad-update/warren-trusted-metadata-signing-pubkeys"

printf '\n%d checks, %d failure(s)\n' "$checks" "$failures"
[ "$failures" -eq 0 ]
