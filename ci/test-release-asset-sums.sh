#!/usr/bin/env bash
# Tests for ci/release-asset-sums.sh: each build job records the hashes of
# what it uploads, and the release's checksum list (which the signed
# manifests and the Linux install script trust) is those hashes, published
# only once the draft holds exactly those bytes.
#
#   bash ci/test-release-asset-sums.sh
set -uo pipefail

SCRIPT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/release-asset-sums.sh"

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
status() { "$@" > /dev/null 2>&1; echo $?; }

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# Two build jobs, each recording what it uploads.
mkdir -p "$work/mac" "$work/linux"
printf 'pkg' > "$work/mac/WarrenVPN-Beta-1.0.0-macos-universal.pkg"
printf 'deb' > "$work/linux/WarrenVPN-Beta-1.0.0-linux-amd64.deb"
printf 'rpm' > "$work/linux/WarrenVPN-Beta-1.0.0-linux-x86_64.rpm"
GITHUB_OUTPUT="$work/out-mac" bash "$SCRIPT" record "$work/mac" > /dev/null 2>&1
check "record succeeds" 0 $?
GITHUB_OUTPUT="$work/out-linux" bash "$SCRIPT" record "$work/linux" > /dev/null 2>&1
# The value between the heredoc delimiters, as a job output carries it.
sums_of() { sed -n '/^sums<</,/^[A-Za-z0-9_]*$/p' "$1" | sed '1d;$d'; }
check "one line per uploaded file" 2 "$(sums_of "$work/out-linux" | wc -l | tr -d ' ')"
check "lines name the file, not its path" "  WarrenVPN-Beta-1.0.0-linux-amd64.deb" \
	"$(sums_of "$work/out-linux" | head -1 | cut -c65-)"
check "an empty upload is refused" 1 \
	"$(mkdir -p "$work/empty" && GITHUB_OUTPUT="$work/out-empty" status bash "$SCRIPT" record "$work/empty")"

{ sums_of "$work/out-mac"; sums_of "$work/out-linux"; } > "$work/expected"

# draft <dir>: what the release holds when publish-release downloads it.
draft() {
	rm -rf "$1"
	mkdir -p "$1"
	cp "$work/mac/"* "$work/linux/"* "$1/"
}

draft "$work/d1"
check "a draft holding exactly the built files passes" 0 \
	"$(status bash "$SCRIPT" check "$work/expected" "$work/d1")"
check "the checksum list is the built hashes" "$(sort -k2 "$work/expected")" \
	"$(cat "$work/d1/SHA256SUMS" 2> /dev/null)"

draft "$work/d2"
printf 'tampered' > "$work/d2/WarrenVPN-Beta-1.0.0-linux-amd64.deb"
check "a draft asset changed after its build is refused" 1 \
	"$(status bash "$SCRIPT" check "$work/expected" "$work/d2")"
check "and no checksum list is written for it" no \
	"$([ -f "$work/d2/SHA256SUMS" ] && echo yes || echo no)"

draft "$work/d3"
printf 'extra' > "$work/d3/WarrenVPN-Beta-1.0.0-linux-extra.deb"
check "a draft asset no build produced is refused" 1 \
	"$(status bash "$SCRIPT" check "$work/expected" "$work/d3")"

draft "$work/d4"
rm "$work/d4/WarrenVPN-Beta-1.0.0-linux-x86_64.rpm"
check "a draft missing a built asset is refused" 1 \
	"$(status bash "$SCRIPT" check "$work/expected" "$work/d4")"

# A re-run finds what the previous run and the metadata job added.
draft "$work/d5"
printf 'old' > "$work/d5/SHA256SUMS"
printf 'sig' > "$work/d5/SHA256SUMS.sshsig"
printf 'torrent' > "$work/d5/WarrenVPN-Beta-1.0.0-linux-amd64.deb.torrent"
check "the list, its signature and the torrents describe a release, they are not built" 0 \
	"$(status bash "$SCRIPT" check "$work/expected" "$work/d5")"

# Git Bash's sha256sum marks every file binary ("hash *name").
sed -E 's/^([0-9a-f]{64})  /\1 */' "$work/expected" > "$work/expected-star"
draft "$work/d6"
check "a Windows-style binary marker is read as the same name" 0 \
	"$(status bash "$SCRIPT" check "$work/expected-star" "$work/d6")"

{ cat "$work/expected"; head -1 "$work/expected"; } > "$work/expected-dup"
draft "$work/d7"
check "two builds claiming one name are refused" 1 \
	"$(status bash "$SCRIPT" check "$work/expected-dup" "$work/d7")"

: > "$work/expected-none"
draft "$work/d8"
check "no recorded hash at all is refused" 1 \
	"$(status bash "$SCRIPT" check "$work/expected-none" "$work/d8")"

echo
echo "$checks checks, $failures failed"
[ "$failures" -eq 0 ]
