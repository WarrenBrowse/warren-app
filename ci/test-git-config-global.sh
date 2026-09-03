#!/usr/bin/env bash
# Tests for ci/git-config-global.sh: the lock wait, the stale-lock removal and
# the pass-through of a real git error.
#
#   bash ci/test-git-config-global.sh
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WRAPPER="$SCRIPT_DIR/git-config-global.sh"

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

fresh_home() {
	local dir
	dir="$(mktemp -d)"
	printf '%s' "$dir"
}

echo "a held lock is waited out"
home="$(fresh_home)"
touch "$home/.gitconfig.lock"
(sleep 2 && rm -f "$home/.gitconfig.lock") &
HOME="$home" WARREN_GIT_CONFIG_ATTEMPTS=10 bash "$WRAPPER" warren.test held >/dev/null 2>&1
check "the write lands once the other writer releases the lock" 0 "$?"
wait
value="$(HOME="$home" git config --global warren.test)"
check "the value written under contention is the one asked for" "held" "$value"
rm -rf "$home"

echo "a stale lock is removed"
home="$(fresh_home)"
touch "$home/.gitconfig.lock"
# Ten minutes old: no writer can still hold it.
touch -t "$(date -v-10M +%Y%m%d%H%M.%S 2>/dev/null || date -d '10 minutes ago' +%Y%m%d%H%M.%S)" "$home/.gitconfig.lock"
# The aging above runs through two `date` dialects. Assert it landed, or the
# two checks below would pass a fresh lock off as a cleared stale one.
[ -n "$(find "$home" -name .gitconfig.lock -mmin +5)" ]
check "the lock under test really is old" 0 "$?"
HOME="$home" WARREN_GIT_CONFIG_ATTEMPTS=5 WARREN_GIT_CONFIG_STALE_SECONDS=60 bash "$WRAPPER" warren.test stale >/dev/null 2>&1
check "the write lands after the stale lock is cleared" 0 "$?"
[ -e "$home/.gitconfig.lock" ]
check "the stale lock is gone" 1 "$?"
rm -rf "$home"

# The runners are not all one platform, and the two `stat` dialects do not
# simply fail on each other's spelling, so the age is read under each in turn
# with a stub on PATH. GNU answering the BSD spelling with a filesystem field
# is what left every Linux runner unable to clear a stale lock.
stub_dir() { # stub_dir <the stat dialect to imitate> <the mtime it reports>
	local dir="$1" dialect="$2" mtime="$3"
	mkdir -p "$dir"
	{
		echo '#!/bin/sh'
		if [ "$dialect" = gnu ]; then
			echo '[ "$1" = "-c" ] && [ "$2" = "%Y" ] && { printf "'"$mtime"'\n"; exit 0; }'
			# `-f` is --file-system here: an answer, and not an mtime.
			echo '[ "$1" = "-f" ] && { printf "%%m\n"; exit 0; }'
		else
			echo '[ "$1" = "-f" ] && [ "$2" = "%m" ] && { printf "'"$mtime"'\n"; exit 0; }'
		fi
		echo 'exit 1'
	} > "$dir/stat"
	chmod +x "$dir/stat"
}

for dialect in gnu bsd; do
	echo "a stale lock is removed under the $dialect stat dialect"
	home="$(fresh_home)"
	bin="$home/bin"
	touch "$home/.gitconfig.lock"
	stub_dir "$bin" "$dialect" "$(($(date +%s) - 600))"
	PATH="$bin:$PATH" HOME="$home" WARREN_GIT_CONFIG_ATTEMPTS=5 \
		WARREN_GIT_CONFIG_STALE_SECONDS=60 bash "$WRAPPER" warren.test stale >/dev/null 2>&1
	check "the write lands after the stale lock is cleared" 0 "$?"
	[ -e "$home/.gitconfig.lock" ]
	check "the stale lock is gone" 1 "$?"
	rm -rf "$home"
done

echo "a lock nobody releases fails after the attempts"
home="$(fresh_home)"
touch "$home/.gitconfig.lock"
HOME="$home" WARREN_GIT_CONFIG_ATTEMPTS=2 WARREN_GIT_CONFIG_STALE_SECONDS=3600 bash "$WRAPPER" warren.test never >/dev/null 2>&1
rc=$?
[ "$rc" -ne 0 ]
check "a live lock that never clears is reported, not looped forever" 0 "$?"
rm -rf "$home"

echo "a real git error is not retried"
home="$(fresh_home)"
HOME="$home" WARREN_GIT_CONFIG_ATTEMPTS=3 bash "$WRAPPER" --no-such-flag >/dev/null 2>&1
rc=$?
[ "$rc" -ne 0 ]
check "an unknown flag fails at once with git's own status" 0 "$?"
rm -rf "$home"

printf '\n%d checks, %d failure(s)\n' "$checks" "$failures"
[ "$failures" -eq 0 ]
