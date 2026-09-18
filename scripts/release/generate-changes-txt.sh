#!/usr/bin/env bash
#
# Write the desktop app's bundled "What's new" screen from the CHANGELOG set.
#
#   bash scripts/release/generate-changes-txt.sh [version]
#   bash scripts/release/generate-changes-txt.sh --check
#
# With no argument the version comes from `dist-assets/desktop-product-version.txt`,
# the same file the GUI reads at build time, so the notes and the version the
# screen displays can only ever come from the same release.
#
# `--check` writes nothing and exits non-zero when a committed file disagrees
# with its changelog. That is what `test/unit/changes-txt.spec.ts` enforces on
# every push, and it is the gate this file exists for: `changes.txt` is
# hand-maintained upstream, and it sat on "First public beta release." from the
# fork until 1.1.29 while the release body and the update prompt were correct,
# because nothing compared it to anything.
#
# One file per language: `changes.txt` from CHANGELOG.md, and
# `changes.<lang>.txt` from each `CHANGELOG.<lang>.md` sibling, which is what
# `readChangelog()` resolves from the app locale. The languages come from the
# siblings on disk, so adding one is adding its changelog. The same sibling set
# feeds the signed update manifest through `ci/build-version-metadata.py`, so
# the installed version's screen and the update prompt cannot disagree.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$REPO_ROOT"

CHANGELOG="CHANGELOG.md"
VERSION_FILE="dist-assets/desktop-product-version.txt"
TARGET="desktop/packages/mullvad-vpn/changes.txt"

check_only=0
version=""
case "${1:-}" in
	--check) check_only=1 ;;
	"") ;;
	-*)
		printf 'usage: %s [version|--check]\n' "${0##*/}" >&2
		exit 2
		;;
	*) version="$1" ;;
esac

if [ -z "$version" ]; then
	[ -f "$VERSION_FILE" ] || { printf 'missing %s\n' "$VERSION_FILE" >&2; exit 1; }
	# A dev build stamps `1.1.29-dev-<hash>`; the changelog is keyed on the
	# release version, so take what precedes the first dash.
	version="$(tr -d '[:space:]' < "$VERSION_FILE" | sed 's/-.*//')"
fi

# "<changelog>|<bundled file>", English first: every other language falls back
# to it at runtime, so it is the one that may never be missing.
sources=("$CHANGELOG|$TARGET")
for sibling in CHANGELOG.*.md; do
	[ -e "$sibling" ] || continue
	lang="${sibling#CHANGELOG.}"
	lang="${lang%.md}"
	sources+=("$sibling|desktop/packages/mullvad-vpn/changes.$lang.txt")
done

failed=0

for source in "${sources[@]}"; do
	changelog="${source%%|*}"
	target="${source#*|}"
	section="$(bash ci/extract-changelog-section.sh "$changelog" "$version")"

	if [ -z "$section" ]; then
		# Refusing here is the point: an empty screen under a version heading
		# is what a user reads as "this update changed nothing".
		printf 'no [%s] section in %s\n' "$version" "$changelog" >&2
		printf 'write the release notes there before building or tagging %s.\n' "$version" >&2
		failed=1
		continue
	fi

	if [ "$check_only" -eq 1 ]; then
		if [ "$section" = "$(cat "$target" 2>/dev/null)" ]; then
			printf '%s is in sync with %s [%s]\n' "$target" "$changelog" "$version"
			continue
		fi
		printf '%s does not match %s [%s]\n\n' "$target" "$changelog" "$version" >&2
		diff -u "$target" - <<< "$section" >&2 || true
		failed=1
		continue
	fi

	printf '%s\n' "$section" > "$target"
	printf 'wrote %s from %s [%s]\n' "$target" "$changelog" "$version"
done

if [ "$failed" -ne 0 ]; then
	[ "$check_only" -eq 1 ] && printf '\nrun: bash scripts/release/generate-changes-txt.sh\n' >&2
	exit 1
fi
