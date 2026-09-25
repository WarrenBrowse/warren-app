#!/bin/sh
#
# Warren VPN for Linux: installs the desktop app on any distribution the
# release ships a package for, from one command.
#
#   curl -fsSL https://beta.warren.ro/install.sh | sh
#
# It asks the machine which package manager it runs (apt, dnf, zypper, yum or
# pacman) and which processor it has, downloads the package made for that pair
# from the update host, checks it against the release's signed SHA256SUMS, and
# hands it to the package manager. From then on the app installs its own
# updates, through the same package manager.
#
#   sh install.sh --dry-run     print what would be installed, install nothing
#
# The release pipeline renders this file (the two @...@ values) and publishes
# it beside the packages it describes. The resolution functions are sourceable
# (WARREN_INSTALL_LIB=1) so ci/test-linux-install.sh checks them against the
# names the pipeline really gives its files.

set -eu

WARREN_UPDATES_URL='@UPDATES_URL@'
WARREN_ARTIFACT_PREFIX='@ARTIFACT_PREFIX@'
WARREN_SUDO=""

err() {
	printf '\033[0;31m[error]\033[0m %s\n' "$*" >&2
	exit 1
}
info() { printf '\033[0;34m[info]\033[0m %s\n' "$*"; }

# ---------------------------------------------------------------------------
# Resolution. No writes and no network.
# ---------------------------------------------------------------------------

# The package format this machine installs, read off the tools it has rather
# than off its distribution's name: a derivative (Pop!_OS, Nobara, EndeavourOS,
# Rocky, Tumbleweed...) keeps its parent's package manager whatever it calls
# itself, and this needs no list of names to go stale.
warren_format() { # warren_format <os-release ID> <init: systemd|other>
	if [ "$1" = nixos ]; then
		echo nixos
	elif command -v apt-get > /dev/null 2>&1 && command -v dpkg > /dev/null 2>&1; then
		# The systemd package's scripts fail on a machine without systemd
		# (Devuan, MX Linux, antiX): those get the sysvinit repack.
		if [ "$2" = systemd ]; then echo deb; else echo deb-sysvinit; fi
	elif command -v rpm > /dev/null 2>&1 \
		&& { command -v dnf > /dev/null 2>&1 || command -v zypper > /dev/null 2>&1 \
			|| command -v yum > /dev/null 2>&1; }; then
		echo rpm
	elif command -v pacman > /dev/null 2>&1; then
		echo pacman
	else
		return 1
	fi
}

# The architecture token the release gives a package of <format>, for the
# processor `uname -m` names. Each packager spells it its own way.
warren_arch_token() { # warren_arch_token <format> <uname -m>
	case "$2" in
		x86_64 | amd64) wat_arch=x64 ;;
		aarch64 | arm64) wat_arch=arm64 ;;
		*) return 1 ;;
	esac
	case "$1:$wat_arch" in
		deb:x64 | deb-sysvinit:x64) echo amd64 ;;
		deb:arm64) echo arm64 ;;
		rpm:x64 | pacman:x64) echo x86_64 ;;
		rpm:arm64 | pacman:arm64) echo aarch64 ;;
		*) return 1 ;;
	esac
}

# The file name of the newest package of <format> for <arch token> that
# <SHA256SUMS> lists.
warren_asset() { # warren_asset <SHA256SUMS> <format> <arch token>
	case "$2" in
		deb) wa_tail="$3.deb" ;;
		deb-sysvinit) wa_tail="$3-sysvinit.deb" ;;
		rpm) wa_tail="$3.rpm" ;;
		pacman) wa_tail="$3.pacman" ;;
		*) return 1 ;;
	esac
	wa_name="$(awk '{ sub(/^\*/, "", $2); print $2 }' "$1" \
		| grep -E "^${WARREN_ARTIFACT_PREFIX}-[0-9]+\.[0-9]+\.[0-9]+-linux-${wa_tail}\$" \
		| sort -t- -k"$(printf '%s' "$WARREN_ARTIFACT_PREFIX" | awk -F- '{ print NF + 1 }')" -V \
		| tail -n 1)"
	[ -n "$wa_name" ] || return 1
	printf '%s\n' "$wa_name"
}

# ---------------------------------------------------------------------------
# Proof of origin. SHA256SUMS comes from the same host as the package, so on
# its own it only proves the download is whole. It is signed with the Warren
# release key (the Ed25519 key that signs the app's update manifests,
# generated offline and held only by the release pipeline), and nothing
# installs unless that signature verifies against the
# key pinned below. The signature is an SSH signature (PROTOCOL.sshsig) made by
# warren-app's ci/sign-headless-sums.sh in the desktop namespace, so a
# signature over the headless CLI's list, or over a manifest, never passes.
# ---------------------------------------------------------------------------

WARREN_SUMS_DOMAIN='warren-desktop-sha256sums/1'
# The same public key twice, in the form each verifier reads. Its hex form is
# the line in mullvad-update/warren-trusted-metadata-signing-pubkeys, which
# ci/test-linux-install.sh checks both of these against.
WARREN_SIGNING_KEY_PEM='-----BEGIN PUBLIC KEY-----
MCowBQYDK2VwAyEAD2hLskWs1aaExGfMybkrtdqiJS7xLommhYO4BuolYKA=
-----END PUBLIC KEY-----'
WARREN_SIGNING_KEY_SSH='ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIA9oS7JFrNWmhMRnzMm5K7XaoiUu8S6JpoWDuAbqJWCg'

# True when this host's openssl verifies an Ed25519 signature over a message.
# Probed rather than read off a version string, and the probe demands a
# refusal as well as an acceptance.
warren_openssl_ed25519() {
	command -v openssl > /dev/null 2>&1 || return 1
	woe_dir="$(mktemp -d)" || return 1
	woe_status=1
	if openssl genpkey -algorithm ed25519 -out "$woe_dir/key" > /dev/null 2>&1 \
		&& openssl pkey -in "$woe_dir/key" -pubout -out "$woe_dir/pub" > /dev/null 2>&1 \
		&& printf 'probe' > "$woe_dir/good" \
		&& printf 'probf' > "$woe_dir/bad" \
		&& openssl pkeyutl -sign -inkey "$woe_dir/key" -rawin \
			-in "$woe_dir/good" -out "$woe_dir/sig" > /dev/null 2>&1 \
		&& openssl pkeyutl -verify -pubin -inkey "$woe_dir/pub" -rawin \
			-in "$woe_dir/good" -sigfile "$woe_dir/sig" > /dev/null 2>&1 \
		&& ! openssl pkeyutl -verify -pubin -inkey "$woe_dir/pub" -rawin \
			-in "$woe_dir/bad" -sigfile "$woe_dir/sig" > /dev/null 2>&1; then
		woe_status=0
	fi
	rm -rf "$woe_dir"
	return "$woe_status"
}

# True when this host's ssh-keygen verifies SSH signatures (OpenSSH 8.1+).
warren_sshsig_capable() {
	command -v ssh-keygen > /dev/null 2>&1 || return 1
	wsc_dir="$(mktemp -d)" || return 1
	wsc_status=1
	if ssh-keygen -q -t ed25519 -N '' -C probe -f "$wsc_dir/key" > /dev/null 2>&1 \
		&& printf 'probe' > "$wsc_dir/good" \
		&& ssh-keygen -q -Y sign -f "$wsc_dir/key" -n probe "$wsc_dir/good" > /dev/null 2>&1 \
		&& printf 'probe %s\n' "$(cat "$wsc_dir/key.pub")" > "$wsc_dir/signers" \
		&& ssh-keygen -Y verify -f "$wsc_dir/signers" -I probe -n probe \
			-s "$wsc_dir/good.sig" < "$wsc_dir/good" > /dev/null 2>&1 \
		&& ! printf 'probf' | ssh-keygen -Y verify -f "$wsc_dir/signers" -I probe -n probe \
			-s "$wsc_dir/good.sig" > /dev/null 2>&1; then
		wsc_status=0
	fi
	rm -rf "$wsc_dir"
	return "$wsc_status"
}

# Verifies an Ed25519 SSH signature with openssl alone, over a blob rebuilt
# here from the pinned namespace and the message, never read from the file.
warren_openssl_verify_sshsig() { # <message> <sshsig> <public key PEM> <work dir>
	# shellcheck disable=SC2059 # the format is the length byte, in octal
	{
		printf 'SSHSIG\000\000\000'
		printf "\\$(printf '%03o' "${#WARREN_SUMS_DOMAIN}")"
		printf '%s' "$WARREN_SUMS_DOMAIN"
		printf '\000\000\000\000\000\000\000\006sha512\000\000\000\100'
		openssl dgst -sha512 -binary < "$1"
	} > "$4/signed" || return 1
	sed '/^-----/d' "$2" | tr -d '\r\n' | openssl base64 -d -A > "$4/sshsig.bin" || return 1
	tail -c 64 "$4/sshsig.bin" > "$4/signature" || return 1
	openssl pkeyutl -verify -pubin -inkey "$3" -rawin \
		-in "$4/signed" -sigfile "$4/signature" > /dev/null 2>&1
}

# Verifies <dir>/SHA256SUMS against <dir>/SHA256SUMS.sshsig and the pinned
# key, with the first verifier this host can run. Says why when it refuses.
warren_verify_sums() { # warren_verify_sums <dir>
	wvs_dir="$1"
	if [ ! -s "$wvs_dir/SHA256SUMS.sshsig" ]; then
		echo "the release carries no SHA256SUMS.sshsig, so nothing proves Warren published it" >&2
		return 1
	fi
	if warren_openssl_ed25519; then
		wvs_tool=openssl
	elif warren_sshsig_capable; then
		wvs_tool=ssh-keygen
	else
		echo "this machine cannot check an Ed25519 signature: install OpenSSL 3 (package openssl) or OpenSSH 8.1 or newer (package openssh-client), then run this again" >&2
		return 1
	fi
	wvs_tmp="$(mktemp -d)" || return 1
	wvs_status=1
	if [ "$wvs_tool" = openssl ]; then
		if printf '%s\n' "$WARREN_SIGNING_KEY_PEM" > "$wvs_tmp/key.pem" \
			&& warren_openssl_verify_sshsig "$wvs_dir/SHA256SUMS" "$wvs_dir/SHA256SUMS.sshsig" \
				"$wvs_tmp/key.pem" "$wvs_tmp"; then
			wvs_status=0
		fi
	elif printf 'warren-release %s\n' "$WARREN_SIGNING_KEY_SSH" > "$wvs_tmp/signers" \
		&& ssh-keygen -Y verify -f "$wvs_tmp/signers" -I warren-release -n "$WARREN_SUMS_DOMAIN" \
			-s "$wvs_dir/SHA256SUMS.sshsig" < "$wvs_dir/SHA256SUMS" > /dev/null 2>&1; then
		wvs_status=0
	fi
	rm -rf "$wvs_tmp"
	[ "$wvs_status" -eq 0 ] \
		|| echo "SHA256SUMS does not carry a valid signature by the Warren release key (checked with $wvs_tool)" >&2
	return "$wvs_status"
}

# The sha256 of <file>, lowercase, from whichever tool this host has.
warren_sha256() { # warren_sha256 <file>
	for ws_tool in sha256sum "shasum -a 256" "openssl dgst -sha256 -r"; do
		# shellcheck disable=SC2086 # the tool carries its own arguments
		ws_out="$($ws_tool "$1" 2> /dev/null)" || continue
		ws_hash="${ws_out%% *}"
		case "$ws_hash" in
			"" | *[!0-9a-fA-F]*) continue ;;
		esac
		[ "${#ws_hash}" -eq 64 ] || continue
		printf '%s\n' "$ws_hash" | tr 'A-F' 'a-f'
		return 0
	done
	return 1
}

# The whole proof for one downloaded package in <dir>: a signed list that
# names it exactly once, with the hash it has.
warren_verify_asset() { # warren_verify_asset <dir> <asset>
	warren_verify_sums "$1" || return 1
	wva_want="$(awk -v a="$2" '$2 == a || $2 == "*" a { print $1 }' "$1/SHA256SUMS" | tr 'A-F' 'a-f')"
	case "$wva_want" in
		"" | *[!0-9a-f]*)
			echo "the signed SHA256SUMS does not list $2 exactly once" >&2
			return 1
			;;
	esac
	if ! wva_got="$(warren_sha256 "$1/$2")"; then
		echo "no sha256sum, shasum or openssl on this machine to checksum $2" >&2
		return 1
	fi
	if [ "$wva_got" != "$wva_want" ]; then
		echo "checksum mismatch: $2 is not the file the signed SHA256SUMS lists" >&2
		return 1
	fi
}

# ---------------------------------------------------------------------------
# Installation.
# ---------------------------------------------------------------------------

# Hands <package> to the package manager for <format>. apt and dnf resolve the
# dependencies a bare dpkg or rpm would leave missing. The package carries no
# GPG signature (its integrity is the signed list checked above), which zypper
# has to be told.
warren_install() { # warren_install <format> <package>
	case "$1" in
		deb | deb-sysvinit) $WARREN_SUDO apt-get install -y "$2" ;;
		rpm)
			if command -v dnf > /dev/null 2>&1; then
				$WARREN_SUDO dnf install -y "$2"
			elif command -v zypper > /dev/null 2>&1; then
				$WARREN_SUDO zypper --non-interactive install --allow-unsigned-rpm "$2"
			else
				$WARREN_SUDO yum install -y "$2"
			fi
			;;
		pacman) $WARREN_SUDO pacman -U --noconfirm "$2" ;;
		*) return 1 ;;
	esac
}

warren_fetch() { # warren_fetch <url> <file>
	if command -v curl > /dev/null 2>&1; then
		curl -fsSL --proto '=https' --tlsv1.2 -o "$2" "$1"
	elif command -v wget > /dev/null 2>&1; then
		wget -q --https-only -O "$2" "$1"
	else
		err "neither curl nor wget is installed"
	fi
}

warren_main() {
	dry_run=0
	for arg in "$@"; do
		case "$arg" in
			--dry-run) dry_run=1 ;;
			*) err "unknown argument: $arg" ;;
		esac
	done

	[ "$(uname -s)" = Linux ] || err "this installer is for Linux; the other systems download the app from the website"

	os_id=""
	if [ -r /etc/os-release ]; then
		# shellcheck disable=SC1091
		os_id="$(. /etc/os-release && printf '%s' "${ID:-}")"
	fi
	init=other
	[ -d /run/systemd/system ] && init=systemd

	format="$(warren_format "$os_id" "$init")" \
		|| err "no supported package manager found (apt, dnf, zypper, yum or pacman): download a package from the website instead"
	if [ "$format" = nixos ]; then
		info "NixOS installs Warren from its flake, declared in your system configuration:"
		info "  ${WARREN_UPDATES_URL} (the -nixos.tar.gz file) and the NixOS section of the install guide"
		exit 1
	fi
	machine="$(uname -m)"
	arch="$(warren_arch_token "$format" "$machine")" \
		|| err "no $format package is published for this processor ($machine)"

	work="$(mktemp -d)"
	# World-readable: apt reads a local package as its unprivileged _apt user.
	chmod 755 "$work"
	trap 'rm -rf "$work"' EXIT
	warren_fetch "$WARREN_UPDATES_URL/SHA256SUMS" "$work/SHA256SUMS" || err "could not reach $WARREN_UPDATES_URL"
	warren_fetch "$WARREN_UPDATES_URL/SHA256SUMS.sshsig" "$work/SHA256SUMS.sshsig" \
		|| err "the release carries no signature for its checksum list"

	# Before anything is read from the list: a list nobody signed names no
	# package worth downloading.
	warren_verify_sums "$work" || err "refusing a checksum list that is not proven to be Warren's"

	asset="$(warren_asset "$work/SHA256SUMS" "$format" "$arch")" \
		|| err "the current release has no $format package for $machine"
	info "Package for this machine: $asset"
	if [ "$dry_run" -eq 1 ]; then
		info "Dry run: the checksum list is signed, nothing downloaded or installed"
		return 0
	fi

	info "Downloading"
	warren_fetch "$WARREN_UPDATES_URL/$asset" "$work/$asset" || err "download failed"
	warren_verify_asset "$work" "$asset" || err "refusing to install a package that is not proven to be Warren's"
	chmod 644 "$work/$asset"

	if [ "$(id -u)" -ne 0 ]; then
		if command -v sudo > /dev/null 2>&1; then
			WARREN_SUDO=sudo
		elif command -v doas > /dev/null 2>&1; then
			WARREN_SUDO=doas
		else
			err "installing a package needs root: run this again as root"
		fi
		info "Installing needs administrator rights, $WARREN_SUDO may ask for your password"
	fi
	warren_install "$format" "$work/$asset" || err "the package manager could not install $asset"
	info "Warren is installed. Open it from your applications menu; it installs its own updates from now on."
}

if [ "${WARREN_INSTALL_LIB:-0}" = "1" ]; then
	return 0 2> /dev/null || exit 0
fi
warren_main "$@"
