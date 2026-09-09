#!/usr/bin/env bash
#
# Write the desktop app's bundled "What's new" screen from CHANGELOG.md.
#
#   bash scripts/release/generate-changes-txt.sh [version]
#   bash scripts/release/generate-changes-txt.sh --check
#
# With no argument the version comes from `dist-assets/desktop-product-version.txt`,
# the same file the GUI reads at build time, so the notes and the version the
# screen displays can only ever come from the same release.
#
# `--check` writes nothing and exits non-zero when the committed
# `changes.txt` disagrees with the changelog. That is what
# `test/unit/changes-txt.spec.ts` enforces on every push, and it is the gate
# this file exists for: `changes.txt` is hand-maintained upstream, and it sat
# on "First public beta release." from the fork until 1.1.29 while the release
# body and the update prompt were correct, because nothing compared it to
# anything.
#
# Scope of what it fixes: the ENGLISH bundled screen. The update prompt shows
# translated notes from the signed manifest (`changelog_translations`), but the
# bundled file is read by `readChangelog()` with no locale, so a translated
# what's-new needs the app to ship one file per language first.
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

section="$(bash ci/extract-changelog-section.sh "$CHANGELOG" "$version")"

if [ -z "$section" ]; then
	# Refusing here is the point: an empty screen under a version heading is
	# what a user reads as "this update changed nothing".
	printf 'no [%s] section in %s\n' "$version" "$CHANGELOG" >&2
	printf 'add the release notes before building or tagging %s.\n' "$version" >&2
	exit 1
fi

if [ "$check_only" -eq 1 ]; then
	if [ "$section" = "$(cat "$TARGET" 2>/dev/null)" ]; then
		printf '%s is in sync with %s [%s]\n' "$TARGET" "$CHANGELOG" "$version"
		exit 0
	fi
	printf '%s does not match %s [%s]\n\n' "$TARGET" "$CHANGELOG" "$version" >&2
	diff -u "$TARGET" - <<< "$section" >&2 || true
	printf '\nrun: bash scripts/release/generate-changes-txt.sh\n' >&2
	exit 1
fi

printf '%s\n' "$section" > "$TARGET"
printf 'wrote %s from %s [%s]\n' "$TARGET" "$CHANGELOG" "$version"
