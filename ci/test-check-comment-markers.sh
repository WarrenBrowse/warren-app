#!/usr/bin/env bash
# Tests for ci/check-comment-markers.sh, which refuses work-tracking markers
# (lot and phase numbers, finding ids, references to numbered documents of a
# private repository) in source files.
#
#   bash ci/test-check-comment-markers.sh
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CHECK="$SCRIPT_DIR/check-comment-markers.sh"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

failures=0
checks=0
check() { # check <description> <command...>
	local description="$1"
	shift
	checks=$((checks + 1))
	if "$@"; then
		printf '  ok   %s\n' "$description"
	else
		printf '  FAIL %s\n' "$description"
		failures=$((failures + 1))
	fi
}

new_repo() { # new_repo <dir>
	mkdir -p "$1"
	git -C "$1" init -q
	git -C "$1" config user.email test@example.invalid
	git -C "$1" config user.name test
}

refuses() { (cd "$1" && shift && ! bash "$CHECK" "$@" > /dev/null 2>&1); }
accepts() { (cd "$1" && shift && bash "$CHECK" "$@" > /dev/null 2>&1); }

for marker in \
	'// docs/59 Lot 3: pre-swap gate' \
	'/// Phase #4 - resolution of the config' \
	'// Phase 2.B: vector tests' \
	'#[test] // (H-1) regression' \
	'// Fix (M-2): zeroize' \
	'/// Convention frozen for the POC:' \
	'// polled on the cadence (warren-core doc 105)' \
	'// see doc 59 for the ladder' \
	'// AUDIT-2026-11-16 finding'; do
	repo="$TMP/tree-$checks"
	new_repo "$repo"
	printf 'fn main() {}\n%s\n' "$marker" > "$repo/lib.rs"
	git -C "$repo" add lib.rs
	check "tree scan refuses: $marker" refuses "$repo"
done

repo="$TMP/clean"
new_repo "$repo"
cat > "$repo/lib.rs" << 'EOF'
// The exit rejected the candidate before the swap, so the ladder advances.
// Phase of the moon, a lot of exits, POCKET, (HTTP-2), document 59 pages.
fn main() {}
EOF
printf 'lot 3 of docs/59 in prose\n' > "$repo/NOTES.md"
git -C "$repo" add lib.rs NOTES.md
check "tree scan accepts ordinary comments and non-source files" accepts "$repo"

repo="$TMP/scoped"
new_repo "$repo"
printf '// Phase 5: in a Kotlin file\n' > "$repo/A.kt"
printf 'fn main() {}\n' > "$repo/lib.rs"
git -C "$repo" add A.kt lib.rs
check "a pathspec limits the scan to the languages it names" accepts "$repo" '*.rs'
check "without a pathspec every source language is scanned" refuses "$repo"

# Staged mode looks only at lines a commit adds, so an untouched legacy line
# does not block an unrelated change.
repo="$TMP/staged"
new_repo "$repo"
printf '// docs/59 Lot 3: legacy line\nfn a() {}\n' > "$repo/lib.rs"
git -C "$repo" add lib.rs
git -C "$repo" commit -q --no-verify -m init
printf '// docs/59 Lot 3: legacy line\nfn a() {}\n// a plain new comment\nfn b() {}\n' > "$repo/lib.rs"
git -C "$repo" add lib.rs
check "staged scan accepts a commit that leaves a legacy marker untouched" accepts "$repo" --staged
printf '// docs/59 Lot 3: legacy line\nfn a() {}\n// Phase 5: new marker\nfn b() {}\n' > "$repo/lib.rs"
git -C "$repo" add lib.rs
check "staged scan refuses a commit that adds a marker" refuses "$repo" --staged

printf '\n%d checks, %d failed\n' "$checks" "$failures"
[ "$failures" -eq 0 ]
