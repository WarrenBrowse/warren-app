#!/usr/bin/env bash
# Tests for ci/extract-changelog-section.sh.
#
#   bash ci/test-extract-changelog-section.sh
#
# This extraction feeds three consumers that must agree: the GitHub release
# body (`release.yml`), the signed update manifests' release notes
# (`ci/build-version-metadata.py`), and the in-app "What's new" screen
# (`changes.txt`, written by `scripts/release/generate-changes-txt.sh`). A
# reader that disagrees with the others ships a release whose notes differ
# depending on where the user reads them, which is how `changes.txt` sat on
# "First public beta release." for 25 releases while every other surface was
# correct.
#
# The prefix trap is the reason this is pinned rather than eyeballed: a
# substring match for "1.1.2" also matches "## [1.1.29]", so a patch release
# would publish a later version's notes.
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
EXTRACT="$SCRIPT_DIR/extract-changelog-section.sh"

failures=0
checks=0

fixture() {
	cat <<'EOF'
# Changelog
Preamble that must never be captured.

## [Unreleased]


## [1.1.29] - 2026-09-09
### Added
- Newest entry.
### Fixed
- [macOS] A tagged entry keeps its tag.

## [1.1.9] - 2026-08-08
### Fixed
- An older entry.

## [1.1.2] - 2026-07-01
### Fixed
- The prefix trap: this section must not answer a query for 1.1.29.
EOF
}

expect() { # expect <description> <version> <expected stdout>
	checks=$((checks + 1))
	local changelog actual
	changelog="$(mktemp)"
	fixture > "$changelog"
	actual="$(bash "$EXTRACT" "$changelog" "$2")"
	rm -f "$changelog"
	if [ "$actual" = "$3" ]; then
		printf '  ok   %s\n' "$1"
	else
		printf '  FAIL %s\n       expected: %s\n       actual:   %s\n' \
			"$1" "$(printf '%s' "$3" | head -3 | tr '\n' '|')" \
			"$(printf '%s' "$actual" | head -3 | tr '\n' '|')"
		failures=$((failures + 1))
	fi
}

printf 'extract-changelog-section\n'

expect 'captures the section body without its own heading' 1.1.29 '### Added
- Newest entry.
### Fixed
- [macOS] A tagged entry keeps its tag.'

expect 'stops at the next version heading' 1.1.9 '### Fixed
- An older entry.'

expect 'takes the last section when it is last in the file' 1.1.2 '### Fixed
- The prefix trap: this section must not answer a query for 1.1.29.'

# The trap this file exists for: a substring reader answers 1.1.2 with the
# 1.1.29 section (it appears first and contains "1.1.2").
expect 'matches the exact version, not a prefix of a later one' 1.1.2 '### Fixed
- The prefix trap: this section must not answer a query for 1.1.29.'

expect 'answers empty for a version that has no section' 9.9.9 ''

expect 'never captures the file preamble' 1.1.29 '### Added
- Newest entry.
### Fixed
- [macOS] A tagged entry keeps its tag.'

# An absent file is not a crash: a release with no changelog still ships, and
# the caller decides what to do with the empty answer.
checks=$((checks + 1))
if [ -z "$(bash "$EXTRACT" /nonexistent/CHANGELOG.md 1.1.29 2>/dev/null)" ]; then
	printf '  ok   %s\n' 'answers empty for a missing changelog file'
else
	printf '  FAIL %s\n' 'answers empty for a missing changelog file'
	failures=$((failures + 1))
fi

# A missing version argument is a caller bug, and silently emitting the whole
# file would put the entire changelog in a release body.
checks=$((checks + 1))
if bash "$EXTRACT" /dev/null > /dev/null 2>&1; then
	printf '  FAIL %s\n' 'refuses a call with no version'
	failures=$((failures + 1))
else
	printf '  ok   %s\n' 'refuses a call with no version'
fi

# The release body keeps the heading; the in-app screen does not.
checks=$((checks + 1))
changelog="$(mktemp)"
fixture > "$changelog"
if [ "$(bash "$EXTRACT" --with-heading "$changelog" 1.1.29 | head -1)" = '## [1.1.29] - 2026-09-09' ]; then
	printf '  ok   %s\n' '--with-heading keeps the version heading, for the release body'
else
	printf '  FAIL %s\n' '--with-heading keeps the version heading, for the release body'
	failures=$((failures + 1))
fi
checks=$((checks + 1))
if [ "$(bash "$EXTRACT" --with-heading "$changelog" 1.1.29 | tail -n +2)" = "$(bash "$EXTRACT" "$changelog" 1.1.29)" ]; then
	printf '  ok   %s\n' '--with-heading changes only the first line'
else
	printf '  FAIL %s\n' '--with-heading changes only the first line'
	failures=$((failures + 1))
fi
rm -f "$changelog"

# Parity with the Python authority that writes the signed manifests' notes.
# Two readers of one file must never answer differently: that is precisely how
# a release ends up with different notes depending on where you read them.
# Skipped where python3 is absent rather than failed: this file's own subject
# is the bash reader.
checks=$((checks + 1))
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
if ! command -v python3 > /dev/null 2>&1; then
	printf '  skip %s (no python3)\n' 'agrees with ci/build-version-metadata.py on the real changelog'
elif version="$(sed -n 's/^## \[\([0-9][0-9.]*\)\].*/\1/p' "$REPO_ROOT/CHANGELOG.md" | head -1)" && [ -n "$version" ] &&
	python_out="$(cd "$REPO_ROOT" && python3 -c '
import importlib.util, pathlib, sys
spec = importlib.util.spec_from_file_location("bvm", "ci/build-version-metadata.py")
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)
sys.stdout.write(m.extract_changelog(pathlib.Path("CHANGELOG.md"), sys.argv[1]))
' "$version")" && [ "$python_out" = "$(bash "$EXTRACT" "$REPO_ROOT/CHANGELOG.md" "$version")" ]; then
	printf '  ok   %s (%s)\n' 'agrees with ci/build-version-metadata.py on the real changelog' "$version"
else
	printf '  FAIL %s\n' 'agrees with ci/build-version-metadata.py on the real changelog'
	failures=$((failures + 1))
fi

printf '\n%d checks, %d failures\n' "$checks" "$failures"
[ "$failures" -eq 0 ]
