#!/usr/bin/env bash
# Refuse a release whose version has no notes in one of the three changelogs.
#
#   bash ci/require-release-notes.sh <version> [<repo-dir>]
#
# Every surface that tells a user what changed reads these files: the GitHub
# release body, the in-app "What's new" screen and the signed update manifests,
# in English, French and Romanian. Eleven beta versions shipped with no notes
# at all, and three more without their translations, so the release job asks
# for all three before it builds anything.
set -euo pipefail

version="${1:-}"
repo="${2:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"

if [ -z "$version" ]; then
	printf 'usage: %s <version> [<repo-dir>]\n' "${0##*/}" >&2
	exit 2
fi

extract="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/extract-changelog-section.sh"
missing=()
for file in CHANGELOG.md CHANGELOG.fr.md CHANGELOG.ro.md; do
	if [ ! -f "$repo/$file" ] || [ -z "$(bash "$extract" "$repo/$file" "$version")" ]; then
		missing+=("$file")
	fi
done

if [ "${#missing[@]}" -gt 0 ]; then
	echo "::error::no release notes for [$version] in: ${missing[*]}" >&2
	exit 1
fi
echo "release notes for [$version] found in the three changelogs"
