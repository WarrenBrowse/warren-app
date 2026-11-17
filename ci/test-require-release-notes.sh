#!/usr/bin/env bash
# Tests for ci/require-release-notes.sh: a release tag is refused unless the
# English, French and Romanian changelogs all carry notes for its version.
#
#   bash ci/test-require-release-notes.sh
set -uo pipefail

SCRIPT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/require-release-notes.sh"

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

dir="$(mktemp -d)"
trap 'rm -rf "$dir"' EXIT

notes() { # notes <file> <body for 1.2.3>
	printf '# Changelog\n\n## [1.2.3] - 2026-11-17\n%s\n\n## [1.2.2] - 2026-11-01\n- Old.\n' \
		"$2" > "$dir/$1"
}
all_notes() {
	notes CHANGELOG.md '- Fix the thing.'
	notes CHANGELOG.fr.md '- Corriger la chose.'
	notes CHANGELOG.ro.md '- Corectează lucrul.'
}

all_notes
check "a version with notes in the three languages passes" 0 \
	"$(status bash "$SCRIPT" 1.2.3 "$dir")"
check "a version absent from every changelog is refused" 1 \
	"$(status bash "$SCRIPT" 1.2.4 "$dir")"

for file in CHANGELOG.md CHANGELOG.fr.md CHANGELOG.ro.md; do
	all_notes
	notes "$file" ''
	check "an empty section in $file is refused" 1 "$(status bash "$SCRIPT" 1.2.3 "$dir")"
	out=$(bash "$SCRIPT" 1.2.3 "$dir" 2>&1)
	check "the refusal names $file" yes \
		"$(case "$out" in *"$file"*) echo yes ;; *) echo no ;; esac)"
done

all_notes
rm "$dir/CHANGELOG.ro.md"
check "a missing translation file is refused" 1 "$(status bash "$SCRIPT" 1.2.3 "$dir")"

check "a missing version argument is a usage error" 2 "$(status bash "$SCRIPT")"

printf '%d checks, %d failed\n' "$checks" "$failures"
[ "$failures" -eq 0 ]
