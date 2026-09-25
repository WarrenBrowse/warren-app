#!/usr/bin/env bash
# Several variables below are set for the sourced installer, which reads them.
# shellcheck disable=SC2034
# Tests for dist-assets/linux/install.sh, the one-command Linux installer the
# download page shows.
#
#   bash ci/test-linux-install.sh
#
# It resolves a file NAME out of the conventions ci/stage-release-assets.sh
# gives the packages, so those names are pinned here, and it refuses anything
# the release key did not sign, so that is checked with a signature made by
# the real signer (ci/sign-headless-sums.sh) under a throwaway key.
# Needs OpenSSL 1.1.1+ and ssh-keygen from OpenSSH 8.1+.
set -uo pipefail
export LC_ALL=C

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INSTALLER="$SCRIPT_DIR/../dist-assets/linux/install.sh"

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
same() { # same <description> <expected> <actual>
	if [ "$2" = "$3" ]; then ok "$1"; else fail "$1 (expected '$2', got '$3')"; fi
}
refuse() { # refuse <description> <command...>
	local description="$1"
	shift
	if "$@" > /dev/null 2>&1; then fail "$description (it succeeded)"; else ok "$description"; fi
}

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

WARREN_INSTALL_LIB=1
# shellcheck source=../dist-assets/linux/install.sh
. "$INSTALLER"
set +e
WARREN_ARTIFACT_PREFIX=WarrenVPN-Beta

# A PATH holding only fakes of <tools>, each recording its argv in $TMP/calls.
fake_path() { # fake_path <dir> <tool...>
	local dir="$1"
	shift
	rm -rf "$dir"
	mkdir -p "$dir"
	for tool in "$@"; do
		printf '#!/bin/sh\necho "%s $*" >> "%s/calls"\n' "$tool" "$TMP" > "$dir/$tool"
		chmod +x "$dir/$tool"
	done
}
format_with() { # format_with <os id> <init> <tool...>
	local id="$1" init="$2"
	shift 2
	fake_path "$TMP/bin" "$@"
	PATH="$TMP/bin" warren_format "$id" "$init"
}

echo "which package format"
same "apt and dpkg install a .deb" deb "$(format_with ubuntu systemd apt-get dpkg)"
same "without systemd, the sysvinit .deb" deb-sysvinit "$(format_with devuan other apt-get dpkg)"
same "a dpkg alone is not an apt system" "" "$(format_with debian systemd dpkg)"
same "dnf and rpm install an .rpm" rpm "$(format_with fedora systemd dnf rpm)"
same "zypper and rpm install an .rpm" rpm "$(format_with opensuse-leap systemd zypper rpm)"
same "yum and rpm install an .rpm" rpm "$(format_with centos systemd yum rpm)"
same "pacman installs a .pacman" pacman "$(format_with archarm systemd pacman)"
same "a derivative is read by its tools, not its name" pacman "$(format_with endeavouros systemd pacman)"
same "NixOS is sent to its flake" nixos "$(format_with nixos systemd)"
refuse "nothing it knows is refused" format_with gentoo systemd emerge

echo "which architecture token"
same "x86_64 .deb" amd64 "$(warren_arch_token deb x86_64)"
same "aarch64 .deb" arm64 "$(warren_arch_token deb aarch64)"
same "x86_64 .rpm" x86_64 "$(warren_arch_token rpm x86_64)"
same "aarch64 .rpm" aarch64 "$(warren_arch_token rpm aarch64)"
same "aarch64 .pacman" aarch64 "$(warren_arch_token pacman aarch64)"
refuse "no arm64 sysvinit package exists" warren_arch_token deb-sysvinit aarch64
refuse "a 32-bit ARM machine has no package" warren_arch_token deb armv7l

echo "which file, from the list a release publishes"
# The names ci/stage-release-assets.sh gives a beta release, as published.
cat > "$TMP/SHA256SUMS" << 'EOF'
d7932384e3474bd1ce347630abb994e97bc1b488f165292024e3de5589ad6a19  WarrenVPN-Beta-1.1.34-linux-aarch64.pacman
09ed76730849838e8a976676690fe26b5a4631263e2a4186a7b5be842290a65a  WarrenVPN-Beta-1.1.34-linux-aarch64.rpm
14c3ea83b8812ac3447274d5b76a7ed702793e93cb5880a8b62347b6cea410aa  WarrenVPN-Beta-1.1.34-linux-amd64-sysvinit.deb
6e87bde74a7b317960d750f547234bcb9d6e895d6f8ec6b2848a44ebdca37df4  WarrenVPN-Beta-1.1.34-linux-amd64.deb
b3d69f02059797dcd278bf1a203355487659f74b0117cb52c6d8e3ea6df8508c  WarrenVPN-Beta-1.1.34-linux-arm64.deb
6d65491e44abfb8f6712762f5158c437c3e8cd64ed7fb0e38a01785d43fa0e4e  WarrenVPN-Beta-1.1.34-linux-x86_64-nixos.tar.gz
cd39fcca5be770420b50762c9aac6c0b1a5c72a92fc9fc44f39b0cf455533716  WarrenVPN-Beta-1.1.34-linux-x86_64.pacman
3f721cf8bad5decf3f54a108a7c3715d81ee48dcbb2f7cefce8afd0c82415b03  WarrenVPN-Beta-1.1.34-linux-x86_64.rpm
4a4b7e63a963de7d21619c202f9c4b8faad2791a4f1348c882315cc805f0176c  WarrenVPN-Beta-1.1.34-windows-x64.exe
EOF
same "x64 .deb" WarrenVPN-Beta-1.1.34-linux-amd64.deb "$(warren_asset "$TMP/SHA256SUMS" deb amd64)"
same "the sysvinit .deb, never the systemd one" WarrenVPN-Beta-1.1.34-linux-amd64-sysvinit.deb \
	"$(warren_asset "$TMP/SHA256SUMS" deb-sysvinit amd64)"
same "arm64 .rpm" WarrenVPN-Beta-1.1.34-linux-aarch64.rpm "$(warren_asset "$TMP/SHA256SUMS" rpm aarch64)"
same "x64 .pacman" WarrenVPN-Beta-1.1.34-linux-x86_64.pacman "$(warren_asset "$TMP/SHA256SUMS" pacman x86_64)"
printf '%s  %s\n' "$(printf 'a%.0s' $(seq 64))" WarrenVPN-Beta-1.1.9-linux-arm64.deb \
	"$(printf 'b%.0s' $(seq 64))" WarrenVPN-Beta-1.1.40-linux-arm64.deb >> "$TMP/SHA256SUMS"
same "the newest version by number, not by spelling" WarrenVPN-Beta-1.1.40-linux-arm64.deb \
	"$(warren_asset "$TMP/SHA256SUMS" deb arm64)"
WARREN_ARTIFACT_PREFIX=WarrenVPN
refuse "a prod installer never takes a beta package" warren_asset "$TMP/SHA256SUMS" deb amd64
WARREN_ARTIFACT_PREFIX=WarrenVPN-Beta

echo "proof of origin"
# Signed by the real signer with a throwaway key, which the installer is told
# to trust in place of the release key (only library mode can do that).
TEST_SEED=7777777777777777777777777777777777777777777777777777777777777777
TEST_PUB=c853ad0f0cd2b619aea92ceec4fd56a24d6499d584ce79257e45cfd8139b60a7
printf '%s\n' "$TEST_PUB" > "$TMP/trusted"
R="$TMP/release"
mkdir -p "$R"
printf 'warren package\n' > "$R/WarrenVPN-Beta-1.2.3-linux-arm64.deb"
(cd "$R" && openssl dgst -sha256 -r WarrenVPN-Beta-1.2.3-linux-arm64.deb \
	| awk '{ sub(/^\*/, "", $2); print $1 "  " $2 }' > SHA256SUMS)
sign() { # sign <namespace>
	(
		WARREN_SIGN_LIB=1 WARREN_SUMS_NAMESPACE="$1"
		export WARREN_SIGN_LIB WARREN_SUMS_NAMESPACE
		# shellcheck source=./sign-headless-sums.sh
		. "$SCRIPT_DIR/sign-headless-sums.sh"
		sign_sums "$R/SHA256SUMS" "$TEST_SEED" "$TMP/trusted"
	) > /dev/null 2>&1
}
sign warren-desktop-sha256sums/1
hex_bytes() { # hex_bytes <hex>: the raw bytes, like the signer writes them
	# shellcheck disable=SC2059
	printf "$(printf '%s' "$1" | sed 's/../\\x&/g')"
}
blob="$( {
	printf '\x00\x00\x00\x0bssh-ed25519\x00\x00\x00\x20'
	hex_bytes "$TEST_PUB"
} | openssl base64 -A)"
WARREN_SIGNING_KEY_SSH="ssh-ed25519 $blob"
# The DER prefix of an Ed25519 SubjectPublicKeyInfo, then the key.
WARREN_SIGNING_KEY_PEM="$(printf -- '-----BEGIN PUBLIC KEY-----\n%s\n-----END PUBLIC KEY-----' "$( {
	printf '\x30\x2a\x30\x05\x06\x03\x2b\x65\x70\x03\x21\x00'
	hex_bytes "$TEST_PUB"
} | openssl base64 -A)")"

if warren_verify_asset "$R" WarrenVPN-Beta-1.2.3-linux-arm64.deb 2> "$TMP/why"; then
	ok "a package the signed list names is accepted"
else
	fail "a package the signed list names is accepted ($(cat "$TMP/why"))"
fi
# Each verifier on its own: a machine may have either one.
if (warren_openssl_ed25519() { return 1; }; warren_verify_sums "$R") 2> /dev/null; then
	ok "ssh-keygen accepts it"
else
	fail "ssh-keygen accepts it"
fi
if warren_openssl_ed25519; then
	if (warren_sshsig_capable() { return 1; }; warren_verify_sums "$R") 2> /dev/null; then
		ok "openssl accepts it"
	else
		fail "openssl accepts it"
	fi
else
	echo "  skip openssl accepts it (this openssl cannot verify Ed25519)"
fi
cp "$R/WarrenVPN-Beta-1.2.3-linux-arm64.deb" "$TMP/good.deb"
printf 'x' >> "$R/WarrenVPN-Beta-1.2.3-linux-arm64.deb"
refuse "a package that differs from the list is refused" warren_verify_asset "$R" WarrenVPN-Beta-1.2.3-linux-arm64.deb
cp "$TMP/good.deb" "$R/WarrenVPN-Beta-1.2.3-linux-arm64.deb"
refuse "a file the list does not name is refused" warren_verify_asset "$R" WarrenVPN-Beta-1.2.3-linux-amd64.deb
sign warren-cli-sha256sums/1
refuse "the CLI's signature never passes for the desktop list" warren_verify_sums "$R"
rm -f "$R/SHA256SUMS.sshsig"
refuse "an unsigned list is refused" warren_verify_sums "$R"

echo "the pinned key is the release key"
release_hex="$(grep -v '^[[:space:]]*#' "$SCRIPT_DIR/../mullvad-update/warren-trusted-metadata-signing-pubkeys" \
	| tr -d ' \t\r' | grep -v '^$' | head -n 1)"
pinned_pem_hex="$(sed -n "/BEGIN/,/END/p" "$INSTALLER" | sed '/-----/d' | tr -d "'\n " \
	| openssl base64 -d -A | tail -c 32 | od -An -tx1 | tr -d ' \n')"
same "the PEM form" "$release_hex" "$pinned_pem_hex"
pinned_ssh_hex="$(sed -n "s/^WARREN_SIGNING_KEY_SSH='ssh-ed25519 \(.*\)'$/\1/p" "$INSTALLER" \
	| openssl base64 -d -A | tail -c 32 | od -An -tx1 | tr -d ' \n')"
same "the SSH form" "$release_hex" "$pinned_ssh_hex"

echo "which package manager runs"
installed() { # installed <format> <tool...>
	local format="$1"
	shift
	fake_path "$TMP/bin" "$@"
	rm -f "$TMP/calls"
	PATH="$TMP/bin" WARREN_SUDO="" warren_install "$format" /tmp/w/pkg > /dev/null
	head -n 1 "$TMP/calls"
}
same "apt resolves a .deb's dependencies" "apt-get install -y /tmp/w/pkg" "$(installed deb apt-get dpkg)"
same "dnf installs an .rpm" "dnf install -y /tmp/w/pkg" "$(installed rpm dnf rpm)"
same "zypper is told the .rpm carries no GPG signature" \
	"zypper --non-interactive install --allow-unsigned-rpm /tmp/w/pkg" "$(installed rpm zypper rpm)"
same "pacman installs without a prompt" "pacman -U --noconfirm /tmp/w/pkg" "$(installed pacman pacman)"

printf '\n%d checks, %d failure(s)\n' "$checks" "$failures"
[ "$failures" -eq 0 ]
