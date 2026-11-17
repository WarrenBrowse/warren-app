#!/usr/bin/env bash
# Tests for the local git hooks in .githooks/: the pre-commit lint guard fixes
# what it reports, file by file, in the directory whose eslint config CI
# applies; and the pre-push format gate formats the crates CI formats.
#
#   bash ci/test-git-hooks.sh
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"

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

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# --- pre-commit: the auto-fix reaches the files the lint refused -----------

repo="$work/repo"
git init -q "$repo"
git -C "$repo" config user.name test
git -C "$repo" config user.email test@example.invalid
mkdir -p "$repo/.githooks" "$repo/desktop/node_modules/.bin" \
	"$repo/desktop/packages/mullvad-vpn/src" "$repo/desktop/scripts"
cp "$ROOT/.githooks/pre-commit" "$repo/.githooks/pre-commit"

# Stands in for eslint: logs "<cwd> <args>" per call, refuses any file that was
# not --fix'ed from that same directory before.
cat > "$repo/desktop/node_modules/.bin/eslint" <<'EOF'
#!/usr/bin/env bash
log="$ESLINT_LOG"
fix=0
files=()
for a in "$@"; do
	case "$a" in
	--fix) fix=1 ;;
	--max-warnings | 0) ;;
	*) files+=("$a") ;;
	esac
done
for f in ${files[@]+"${files[@]}"}; do
	if [ "$fix" -eq 1 ]; then
		echo "fixed $PWD/$f" >> "$log"
	elif ! grep -qxF "fixed $PWD/$f" "$log" 2> /dev/null; then
		exit 1
	fi
done
exit 0
EOF
chmod +x "$repo/desktop/node_modules/.bin/eslint"
echo 'x' > "$repo/desktop/packages/mullvad-vpn/src/a.ts"
echo 'x' > "$repo/desktop/scripts/b.ts"
git -C "$repo" add desktop/packages/mullvad-vpn/src/a.ts desktop/scripts/b.ts

out="$(cd "$repo" && ESLINT_LOG="$work/eslint.log" bash .githooks/pre-commit 2>&1)"
check "a lint failure still aborts the commit" 1 $?
real_repo="$(cd "$repo" && pwd -P)"
check "the package file is fixed under the package's config" yes \
	"$(grep -qxF "fixed $real_repo/desktop/packages/mullvad-vpn/src/a.ts" "$work/eslint.log" 2> /dev/null && echo yes || echo no)"
check "a desktop-root file is fixed under the root config" yes \
	"$(grep -qxF "fixed $real_repo/desktop/scripts/b.ts" "$work/eslint.log" 2> /dev/null && echo yes || echo no)"
told=no
case "$out" in *Auto-fixed*) told=yes ;; esac
check "the developer is told the tree was fixed" yes "$told"

# --- pre-push: the format gate's crates are CI's crates -----------------------

# fmt_scope <file>: the crates a `cargo fmt` invocation in <file> formats, one
# per line, sorted. A reference to the shared list counts as that list.
fmt_scope() {
	local block
	# Code lines only, so a comment naming the list cannot stand in for using it.
	block="$(grep -vE '^[[:space:]]*#' "$1" |
		awk '/cargo fmt/ && !/name:/ { on = 1 } on { print } on && /-- --check/ { exit }')"
	if printf '%s\n' "$block" | grep -q 'ci/rust-fmt-scope.txt'; then
		grep -vE '^\s*(#|$)' "$ROOT/ci/rust-fmt-scope.txt"
	else
		printf '%s\n' "$block" | grep -oE -- '-p [a-z0-9-]+' | cut -c4-
	fi | sort -u
}
ci_scope="$(fmt_scope "$ROOT/.github/workflows/warren-tests.yml")"
hook_scope="$(fmt_scope "$ROOT/.githooks/pre-push")"
check "CI formats a Warren scope" yes "$([ -n "$ci_scope" ] && echo yes || echo no)"
check "pre-push formats exactly the crates CI formats" "$ci_scope" "$hook_scope"

echo
echo "$checks checks, $failures failed"
[ "$failures" -eq 0 ]
