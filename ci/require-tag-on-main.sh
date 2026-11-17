#!/usr/bin/env bash
# Refuses to release a commit that main does not carry.
#
#   bash ci/require-tag-on-main.sh <commit>
#
# Run from a checkout whose origin/main is current. The version guard only
# compares numbers, so without this a tag on a side branch, or on a commit
# pushed nowhere else, would be built, signed and served to every client.
set -euo pipefail

commit="${1:?usage: $0 <commit>}"
if ! git merge-base --is-ancestor "$commit" origin/main 2> /dev/null; then
	echo "::error::$commit is not on main: a release is cut from a commit main already carries" >&2
	exit 1
fi
echo "$commit is on main"
