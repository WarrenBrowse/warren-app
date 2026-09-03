#!/usr/bin/env bash
# `git config --global` that survives a shared home directory.
#
#   ci/git-config-global.sh <git config arguments...>
#
# The Windows CI runners are three runner services inside ONE VM, all running
# as the same user, so every job's `git config --global` writes the same
# ~/.gitconfig. git takes ~/.gitconfig.lock for the write and refuses when the
# lock exists ("could not lock config file ...: File exists"), which is what
# killed the beta-v1.1.23 Windows build while the headless bundle job wrote its
# own keys a few milliseconds earlier. The write itself lasts milliseconds, so
# waiting is the whole fix; a lock older than a minute is a crashed job's and
# is removed, because nothing will ever release it.
#
# The bundled git on the Rosetta runner is 2.30, so GIT_CONFIG_GLOBAL (2.32) is
# not available as a per-job alternative.
set -uo pipefail

ATTEMPTS="${WARREN_GIT_CONFIG_ATTEMPTS:-30}"
STALE_SECONDS="${WARREN_GIT_CONFIG_STALE_SECONDS:-60}"

lock_file() {
	local home="${HOME:-}"
	[ -n "$home" ] || return 1
	printf '%s/.gitconfig.lock' "$home"
}

lock_age_seconds() { # lock_age_seconds <path>
	local now mtime
	now="$(date +%s)"
	# The two stat dialects do not simply fail on each other's spelling: GNU's
	# `-f` is --file-system, so it ANSWERS the BSD `-f %m` with a filesystem
	# field and exit 0. Asking BSD first therefore read a non-number as the
	# lock's age on every Linux runner, and no stale lock was ever cleared. So
	# take the first answer that is a plain count of seconds rather than trust
	# an exit status.
	for mtime in "$(stat -c %Y "$1" 2> /dev/null)" "$(stat -f %m "$1" 2> /dev/null)"; do
		case "$mtime" in
			'' | *[!0-9]*) continue ;;
		esac
		printf '%s' "$((now - mtime))"
		return 0
	done
	return 1
}

attempt=1
while :; do
	# The status is read from the command substitution itself: after an `if`
	# whose condition failed, `$?` is the if statement's own 0.
	output="$(git config --global "$@" 2>&1)"
	rc=$?
	if [ "$rc" -eq 0 ]; then
		[ -n "$output" ] && printf '%s\n' "$output"
		exit 0
	fi
	if ! printf '%s' "$output" | grep -q "could not lock config file"; then
		printf '%s\n' "$output" >&2
		exit "$rc"
	fi
	if [ "$attempt" -ge "$ATTEMPTS" ]; then
		printf '%s\n' "$output" >&2
		echo "git config --global: still locked after $attempt attempts" >&2
		exit "$rc"
	fi
	lock="$(lock_file)" || true
	if [ -n "${lock:-}" ] && [ -e "$lock" ]; then
		age="$(lock_age_seconds "$lock" || echo 0)"
		if [ "$age" -ge "$STALE_SECONDS" ]; then
			echo "git config --global: removing a stale lock (${age}s old, no writer can still hold it)" >&2
			rm -f "$lock"
		fi
	fi
	sleep 1
	attempt=$((attempt + 1))
done
