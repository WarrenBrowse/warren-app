#!/usr/bin/env bash
# Tests for ci/publish-to-update-host.sh against a stand-in for the update
# host's receiver (warren-updates-receive: list, put, prune), reached through
# a stub `ssh` on PATH that records how it was called.
#
#   bash ci/test-publish-to-update-host.sh
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SCRIPT="$SCRIPT_DIR/publish-to-update-host.sh"

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

# A work tree with the release files, the signed manifests, the stub ssh and
# the receiver's root directory.
setup() { # setup <version> <prefix>
	work="$(mktemp -d)"
	mkdir -p "$work/release-files" "$work/signed" "$work/bin" "$work/host"
	local f
	for f in "$2-$1-x86_64.deb" "$2-$1-macos-universal.pkg" "$2-$1-android-mapping.txt.gz"; do
		printf '%s' "installer $f" > "$work/release-files/$f"
	done
	for f in SHA256SUMS SHA256SUMS.sshsig install.sh; do
		printf '%s' "$f" > "$work/release-files/$f"
	done
	printf '{}' > "$work/signed/linux.json"
	printf '{}' > "$work/signed/macos.json"
	cat > "$work/bin/ssh" << 'STUB'
#!/usr/bin/env bash
# Records the options, then plays the receiver with the last argument.
printf '%s\n' "$*" >> "$STUB_LOG"
command="${!#}"
set -- $command
[ "$1" = warren-updates-receive ] || { echo "not the receiver: $command" >&2; exit 2; }
verb="$2" dir="$3"
case "$dir" in desktop | desktop/beta) ;; *) echo "bad dir $dir" >&2; exit 2 ;; esac
root="$STUB_HOST/$dir"
case "$verb" in
list) [ -d "$root" ] && ls -ln "$root" ;;
put)
	[ -n "${STUB_FAIL_PUT:-}" ] && { cat > /dev/null; exit 1; }
	mkdir -p "$root" && tar -xf - -C "$root"
	;;
prune)
	for f in "$root"/*; do
		name="${f##*/}"
		case "$name" in WarrenVPN*.pkg | WarrenVPN*.deb | WarrenVPN*.exe) ;; *) continue ;; esac
		case "$name" in "$4"*) ;; *) rm -f -- "$f" ;; esac
	done
	;;
*) exit 2 ;;
esac
STUB
	chmod +x "$work/bin/ssh"
}

publish() { # publish <channel> <version> <prefix>
	(
		cd "$work" &&
			PATH="$work/bin:$PATH" STUB_LOG="$work/ssh.log" STUB_HOST="$work/host" \
				WARREN_UPDATES_SSH_KEY="not-a-key" WARREN_UPDATES_SSH_HOST=updates.example \
				WARREN_UPDATES_SSH_USER=warren-deploy WARREN_CHANNEL="$1" \
				bash "$SCRIPT" release-files signed "$3" "$2"
	) > "$work/out" 2>&1
}

echo "a beta release lands in desktop/beta, manifests included"
setup 1.2.3 WarrenVPN-Beta
mkdir -p "$work/host/desktop/beta" &&
	printf 'old' > "$work/host/desktop/beta/WarrenVPN-Beta-1.2.2-macos-universal.pkg"
publish beta 1.2.3 WarrenVPN-Beta
check "the script succeeds" 0 "$?"
for f in WarrenVPN-Beta-1.2.3-x86_64.deb WarrenVPN-Beta-1.2.3-macos-universal.pkg SHA256SUMS SHA256SUMS.sshsig install.sh linux.json macos.json; do
	[ -f "$work/host/desktop/beta/$f" ]
	check "$f is on the host" 0 "$?"
done
[ -e "$work/host/desktop/beta/WarrenVPN-Beta-1.2.3-android-mapping.txt.gz" ]
check "the R8 mapping stays off the host" 1 "$?"
[ -e "$work/host/desktop/beta/WarrenVPN-Beta-1.2.2-macos-universal.pkg" ]
check "the previous version's installer is pruned" 1 "$?"
[ -e "$work/host/desktop/linux.json" ]
check "nothing lands in the prod directory" 1 "$?"
grep -q 'prune desktop/beta WarrenVPN-Beta-1.2.3-$' "$work/ssh.log"
check "the prune keeps exactly this version" 0 "$?"
last_put="$(grep -n 'put desktop/beta' "$work/ssh.log" | tail -1 | cut -d: -f1)"
prune_line="$(grep -n 'prune' "$work/ssh.log" | cut -d: -f1)"
[ "$last_put" -lt "$prune_line" ]
check "the prune comes after every upload" 0 "$?"
grep -v -q 'StrictHostKeyChecking=yes' "$work/ssh.log"
check "every connection checks the host key" 1 "$?"
grep -q 'accept-new\|StrictHostKeyChecking=no' "$work/ssh.log"
check "no connection trusts an unknown host key" 1 "$?"
grep -v -q 'HostKeyAlias=warren-updates' "$work/ssh.log"
check "every connection pins the update host's key" 1 "$?"
grep -q -- '-o UserKnownHostsFile=.*update-host.known_hosts' "$work/ssh.log"
check "the pinned keys come from the repository" 0 "$?"
grep -q 'not-a-key' "$work/out"
check "the private key never reaches the log" 1 "$?"
rm -rf "$work"

echo "an installer already on the host is not sent again"
setup 1.2.3 WarrenVPN-Beta
mkdir -p "$work/host/desktop/beta"
cp "$work/release-files/WarrenVPN-Beta-1.2.3-macos-universal.pkg" "$work/host/desktop/beta/"
publish beta 1.2.3 WarrenVPN-Beta
check "the script succeeds" 0 "$?"
grep -q 'WarrenVPN-Beta-1.2.3-macos-universal.pkg' "$work/out" && ! grep -q 'streamed WarrenVPN-Beta-1.2.3-macos-universal.pkg' "$work/out"
check "the present installer is skipped" 0 "$?"
grep -q 'streamed WarrenVPN-Beta-1.2.3-x86_64.deb' "$work/out"
check "the missing one is streamed" 0 "$?"
rm -rf "$work"

echo "a prod release lands in desktop"
setup 2.0.0 WarrenVPN
publish prod 2.0.0 WarrenVPN
check "the script succeeds" 0 "$?"
[ -f "$work/host/desktop/WarrenVPN-2.0.0-macos-universal.pkg" ] && [ -f "$work/host/desktop/macos.json" ]
check "installer and manifest are in desktop" 0 "$?"
[ -e "$work/host/desktop/beta" ]
check "the beta directory is untouched" 1 "$?"
rm -rf "$work"

echo "a failed upload publishes no manifest"
setup 1.2.3 WarrenVPN-Beta
(
	export STUB_FAIL_PUT=1
	publish beta 1.2.3 WarrenVPN-Beta
)
check "the script fails" 1 "$?"
[ -e "$work/host/desktop/beta/linux.json" ]
check "no manifest points at a missing installer" 1 "$?"
grep -q 'prune' "$work/ssh.log"
check "nothing is pruned" 1 "$?"
rm -rf "$work"

echo "an unknown channel is refused"
setup 1.2.3 WarrenVPN
publish staging 1.2.3 WarrenVPN
check "the script fails" 1 "$?"
[ -s "$work/ssh.log" ]
check "before connecting" 1 "$?"
rm -rf "$work"

echo
echo "$checks checks, $failures failed"
[ "$failures" = 0 ]
