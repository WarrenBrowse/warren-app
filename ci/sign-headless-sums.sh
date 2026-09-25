#!/usr/bin/env bash
# Signs the SHA256SUMS of a warren-cli headless release, or of a desktop
# release, with the Warren update key, which is what lets the installers prove
# the release came from us.
#
#   WARREN_UPDATE_SIGNING_KEY=<64-hex seed> bash ci/sign-headless-sums.sh <SHA256SUMS>
#
# SHA256SUMS is published in the same release as the packages it lists, so on
# its own it proves only that a download is whole. The installers
# (warren-cli scripts/install.sh and windows/install-windows.ps1) run as root
# or Administrator and refuse any release whose list carries no signature by
# the key they pin. The signature is written next to the list as
# SHA256SUMS.sshsig, an SSH signature (PROTOCOL.sshsig) in namespace
# warren-cli-sha256sums/1, made by `ssh-keygen -Y sign` itself. The installers
# check it with `ssh-keygen -Y verify` (OpenSSH 8.1+: macOS, Windows, most
# Linux) or with OpenSSL 3, which verifies the Ed25519 signature inside it.
#
# The namespace binds the signature to this purpose: the update key also signs
# the app's update manifests, and a signature over one can never pass for the
# other. The key must be one of
# mullvad-update/warren-trusted-metadata-signing-pubkeys, and the signature is
# verified before this returns. Needs OpenSSH 8.1+ and OpenSSL 1.1.1+.
set -euo pipefail
export LC_ALL=C

# The desktop release signs its own list under warren-desktop-sha256sums/1
# (WARREN_SUMS_NAMESPACE), which its Linux install script verifies.
SUMS_DOMAIN="${WARREN_SUMS_NAMESPACE:-warren-cli-sha256sums/1}"

die() {
	echo "sign-headless-sums: $*" >&2
	exit 1
}

# Four bytes, big-endian: the length prefix of an SSH wire string.
u32() {
	# shellcheck disable=SC2059 # the format IS the escaped bytes
	printf "$(printf '\\x%02x\\x%02x\\x%02x\\x%02x' \
		$(($1 >> 24 & 255)) $(($1 >> 16 & 255)) $(($1 >> 8 & 255)) $(($1 & 255)))"
}
ssh_string_text() { # ssh_string_text <ASCII text>
	u32 "${#1}"
	printf '%s' "$1"
}
ssh_string_file() { # ssh_string_file <file>
	u32 "$(wc -c < "$1" | tr -d ' ')"
	cat "$1"
}
hex_to_file() { # hex_to_file <hex> <out>
	# shellcheck disable=SC2059
	printf "$(printf '%s' "$1" | sed 's/../\\x&/g')" > "$2"
}

# Overwrites then removes files that held key material.
destroy() { # destroy <file...>
	shred -u -- "$@" 2> /dev/null || rm -f -- "$@"
}

# The raw 32-byte public key of a PKCS#8 private key, as lowercase hex.
public_key_hex() { # public_key_hex <key.pem>
	openssl pkey -in "$1" -pubout -outform DER | tail -c 32 | od -An -tx1 | tr -d ' \n'
}

# An unencrypted OpenSSH private key (PROTOCOL.key) for an Ed25519 seed and
# its public half, the only form `ssh-keygen -Y sign` reads the key in.
openssh_private_key() { # openssh_private_key <seed file> <public key file> <out>
	local dir
	dir="$(dirname "$3")"
	{ ssh_string_text ssh-ed25519 && ssh_string_file "$2"; } > "$dir/pub.blob"
	cat "$1" "$2" > "$dir/secret"
	{
		u32 1
		u32 1
		ssh_string_text ssh-ed25519
		ssh_string_file "$2"
		ssh_string_file "$dir/secret"
		ssh_string_text warren-release
	} > "$dir/private"
	local length pad=1
	length="$(wc -c < "$dir/private" | tr -d ' ')"
	while [ $((length % 8)) -ne 0 ]; do
		# shellcheck disable=SC2059
		printf "$(printf '\\x%02x' "$pad")" >> "$dir/private"
		pad=$((pad + 1))
		length=$((length + 1))
	done
	{
		printf 'openssh-key-v1\0'
		ssh_string_text none
		ssh_string_text none
		ssh_string_text ''
		u32 1
		ssh_string_file "$dir/pub.blob"
		ssh_string_file "$dir/private"
	} > "$dir/key.bin"
	{
		echo '-----BEGIN OPENSSH PRIVATE KEY-----'
		openssl base64 -A < "$dir/key.bin" | tr -d '\n' | fold -w 70
		echo
		echo '-----END OPENSSH PRIVATE KEY-----'
	} > "$3"
	destroy "$dir/secret" "$dir/private" "$dir/key.bin"
}

# Signs <sums> with the Ed25519 <seed hex> into <sums>.sshsig, refusing a key
# that <trusted keys file> does not list.
#
# A subshell, so its EXIT trap removes the key material on every way out.
sign_sums() ( # sign_sums <sums> <seed hex> <trusted keys file>
	sums="$1"
	seed="$2"
	trusted="$3"
	[ -s "$sums" ] || die "no checksum list at $sums"
	[[ "$seed" =~ ^[0-9a-fA-F]{64}$ ]] || die "the signing key must be a 64-hex Ed25519 seed"

	# The seed is on disk only inside this 0700 directory, in memory where the
	# runner has a tmpfs, for as long as the tools need it.
	work="$(umask 077 && { mktemp -d -p /dev/shm 2> /dev/null || mktemp -d; })"
	trap 'find "$work" -type f -exec shred -u {} + 2> /dev/null || true; rm -rf "$work"' EXIT
	umask 077

	hex_to_file "302e020100300506032b657004220420$seed" "$work/key.der"
	openssl pkey -inform DER -in "$work/key.der" -out "$work/key.pem" 2> /dev/null \
		|| die "openssl cannot load the signing key"
	pub="$(public_key_hex "$work/key.pem")"
	grep -v '^[[:space:]]*#' "$trusted" | tr -d ' \t\r' | grep -qx "$pub" \
		|| die "the signing key is not one of $trusted"

	hex_to_file "$seed" "$work/seed"
	hex_to_file "$pub" "$work/pub"
	openssh_private_key "$work/seed" "$work/pub" "$work/id_ed25519"
	cp "$sums" "$work/SHA256SUMS"
	ssh-keygen -q -Y sign -f "$work/id_ed25519" -n "$SUMS_DOMAIN" "$work/SHA256SUMS" > /dev/null 2>&1 \
		|| die "ssh-keygen cannot sign (OpenSSH 8.1 or newer is required)"

	printf 'warren-release ssh-ed25519 %s\n' "$(openssl base64 -A < "$work/pub.blob" | tr -d '\n')" \
		> "$work/signers"
	ssh-keygen -Y verify -f "$work/signers" -I warren-release -n "$SUMS_DOMAIN" \
		-s "$work/SHA256SUMS.sig" < "$sums" > /dev/null \
		|| die "the signature does not verify"

	umask 022
	cp "$work/SHA256SUMS.sig" "$sums.sshsig"
	echo "signed $sums" >&2
)

# Sourced by ci/test-sign-headless-sums.sh, which wants the functions only.
if [ "${WARREN_SIGN_LIB:-0}" = "1" ]; then
	return 0 2> /dev/null || exit 0
fi

[ "$#" -eq 1 ] || die "usage: WARREN_UPDATE_SIGNING_KEY=<hex> $0 <SHA256SUMS>"
[ -n "${WARREN_UPDATE_SIGNING_KEY:-}" ] || die "WARREN_UPDATE_SIGNING_KEY is not set: a release cannot be published unsigned"
# Out of the environment before any tool runs: every child would inherit it.
seed="$WARREN_UPDATE_SIGNING_KEY"
unset WARREN_UPDATE_SIGNING_KEY
repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
sign_sums "$1" "$seed" "$repo/mullvad-update/warren-trusted-metadata-signing-pubkeys"
