#!/usr/bin/env bash
# Print the changelog section for one version.
#
#   bash ci/extract-changelog-section.sh [--with-heading] <changelog-file> <version>
#
# The single reader shared by every surface that shows release notes: the
# GitHub release body (`release.yml`, which wants the `## [x.y.z] - date`
# heading kept) and the in-app "What's new" screen through
# `scripts/release/generate-changes-txt.sh` (which does not, because the view
# already displays the version). They used to carry their own copies of this
# logic and drifted.
#
# `ci/build-version-metadata.py` holds the fourth surface, the signed
# manifests, in Python. `ci/test-extract-changelog-section.sh` pins the two
# implementations to the same answer on the real changelog, so neither can
# drift alone.
#
# Matching is on the bracketed version (`## [1.1.29] - 2026-09-09`), never a
# substring: "1.1.2" appears inside "1.1.29", so a substring reader answers a
# patch release with a later version's notes.
#
# An absent file or an unknown version prints nothing and exits 0. A release
# with no notes is legitimate (`release.yml` falls back to auto-generated
# ones); the caller decides. A missing version argument is a caller bug and
# exits 2, because emitting the whole changelog would put every version's
# notes in one release body.
set -euo pipefail

with_heading=0
if [ "${1:-}" = "--with-heading" ]; then
	with_heading=1
	shift
fi

changelog="${1:-}"
version="${2:-}"

if [ -z "$changelog" ] || [ -z "$version" ]; then
	printf 'usage: %s [--with-heading] <changelog-file> <version>\n' "${0##*/}" >&2
	exit 2
fi

[ -f "$changelog" ] || exit 0

# `cap` turns on at the heading holding "[<version>]" and off at the next
# `## ` heading, so the body is everything between them. The heading itself is
# printed only for the release body: every other consumer displays the version
# on its own.
awk -v want="[$version]" -v keep_heading="$with_heading" '
	/^## / {
		if (cap) { exit }
		cap = index($0, want) > 0
		if (cap && keep_heading) { print }
		next
	}
	cap { print }
' "$changelog" | awk '
	# Trim leading and trailing blank lines without collapsing the interior.
	{ lines[NR] = $0 }
	END {
		first = 1; last = NR
		while (first <= last && lines[first] ~ /^[[:space:]]*$/) { first++ }
		while (last >= first && lines[last] ~ /^[[:space:]]*$/) { last-- }
		for (i = first; i <= last; i++) { print lines[i] }
	}
'
