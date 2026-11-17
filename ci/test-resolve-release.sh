#!/usr/bin/env bash
# Tests for the release workflow's first two decisions: which channel a ref
# releases (ci/resolve-release-channel.sh), and whether the tagged commit is
# one main already carries (ci/require-tag-on-main.sh).
#
#   bash ci/test-resolve-release.sh
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RESOLVE="$HERE/resolve-release-channel.sh"
ON_MAIN="$HERE/require-tag-on-main.sh"

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
field() { # field <key> <ref>
	bash "$RESOLVE" "$2" 2> /dev/null | sed -n "s/^$1=//p"
}

check "a v tag releases prod" prod "$(field channel v1.2.3)"
check "its version drops the v" 1.2.3 "$(field version v1.2.3)"
check "prod builds against the prod API" https://api.warrenbrowse.com "$(field api-url v1.2.3)"
check "a beta-v tag releases beta" beta "$(field channel beta-v1.1.57)"
check "its version drops the channel prefix" 1.1.57 "$(field version beta-v1.1.57)"
check "beta builds against the beta API" https://api.beta.warrenbrowse.com "$(field api-url beta-v1.1.57)"
check "a branch is no release and is refused" 1 "$(status bash "$RESOLVE" main)"
check "a headless tag is not an app release" 1 "$(status bash "$RESOLVE" daemon-v1.2.3)"
check "no ref is refused" 1 "$(status bash "$RESOLVE" "")"

# A repository with main, a commit on main, and a commit only on a side branch.
repo="$(mktemp -d)"
trap 'rm -rf "$repo"' EXIT
git -C "$repo" init -q -b main
git -C "$repo" config user.name test
git -C "$repo" config user.email test@example.invalid
# A developer's global hooks have no say over this scratch repository.
git -C "$repo" config core.hooksPath /dev/null
git -C "$repo" commit -q --allow-empty -m one
git -C "$repo" commit -q --allow-empty -m two
on_main="$(git -C "$repo" rev-parse HEAD)"
git -C "$repo" checkout -q -b side
git -C "$repo" commit -q --allow-empty -m side
off_main="$(git -C "$repo" rev-parse HEAD)"
git -C "$repo" checkout -q main
# The guard reads origin/main, as a checkout of the workflow sees it.
git -C "$repo" update-ref refs/remotes/origin/main "$on_main"

check "a commit main carries may be released" 0 \
	"$(cd "$repo" && status bash "$ON_MAIN" "$on_main")"
check "an older commit of main may be released" 0 \
	"$(cd "$repo" && status bash "$ON_MAIN" "$on_main~1")"
check "a commit main does not carry is refused" 1 \
	"$(cd "$repo" && status bash "$ON_MAIN" "$off_main")"
check "an unknown commit is refused" 1 \
	"$(cd "$repo" && status bash "$ON_MAIN" 0123456789abcdef0123456789abcdef01234567)"

echo
echo "$checks checks, $failures failed"
[ "$failures" -eq 0 ]
