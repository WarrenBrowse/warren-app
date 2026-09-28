#!/usr/bin/env bash
# Mechanical half of an upstream review (docs/UPSTREAM-DETACH.md): lists what
# mullvad/mullvadvpn-app changed since the last reviewed commit, and sorts it
# for a human or an agent to judge. Read-only: it fetches the `upstream`
# remote (adding it if the clone lacks it) and only ever runs
# `git apply --check`, so the working tree and the index are never touched.
#
# Usage: scripts/dev/upstream-triage.sh [FROM [TO]]
#   FROM defaults to the commit in .upstream-reviewed, TO to upstream/main.
#
# Verdicts for a watch-list commit:
#   PICKED    a commit here names it as "(upstream <sha10>)"
#   CLEAN     its watch-list part applies on the working tree as is
#   PRESENT   its watch-list part applies in reverse: the code is already here
#   CONFLICT  neither; the fork diverged, so read the diff and judge by hand
set -euo pipefail

REPO_ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
cd "$REPO_ROOT"

UPSTREAM_URL="https://github.com/mullvad/mullvadvpn-app.git"
WATCH_FILE="scripts/dev/upstream-watch-paths.txt"
KEYWORDS='GHSA-|CVE-|leak|LPE|privilege|escalat|vulnerab|advisory|secur|setuid|permissive'

if ! git remote get-url upstream >/dev/null 2>&1; then
    git remote add upstream "$UPSTREAM_URL"
fi
git fetch --quiet --no-tags upstream main

FROM="${1:-$(tr -d '[:space:]' < .upstream-reviewed)}"
TO="${2:-upstream/main}"
FROM_SHA="$(git rev-parse --verify "$FROM^{commit}")"
TO_SHA="$(git rev-parse --verify "$TO^{commit}")"

WATCH=()
while IFS= read -r line; do
    line="${line%%#*}"
    line="$(echo "$line" | xargs)"
    [[ -n "$line" ]] && WATCH+=("$line")
done < "$WATCH_FILE"

PICKED="$(git log --format=%s HEAD | grep -oE '\(upstream [0-9a-f]{10}(, [0-9a-f]{10})*\)' | grep -oE '[0-9a-f]{10}' || true)"

echo "Upstream range: ${FROM_SHA:0:10} ($(git show -s --format=%cs "$FROM_SHA")) .. ${TO_SHA:0:10} ($(git show -s --format=%cs "$TO_SHA"))"
echo "Commits in range: $(git rev-list --count --no-merges "$FROM_SHA..$TO_SHA") (merges excluded)"
echo

echo "== 1. Commits touching the watch list ($WATCH_FILE)"
for sha in $(git log --no-merges --reverse --format=%h --abbrev=10 "$FROM_SHA..$TO_SHA" -- "${WATCH[@]}"); do
    patch="$(git format-patch -1 --stdout "$sha" -- "${WATCH[@]}")"
    if grep -qx "$sha" <<<"$PICKED"; then
        verdict=PICKED
    elif git apply --check <<<"$patch" 2>/dev/null; then
        verdict=CLEAN
    elif git apply --check -R <<<"$patch" 2>/dev/null; then
        verdict=PRESENT
    else
        verdict=CONFLICT
    fi
    printf '%s %-8s %s %s\n' "$sha" "$verdict" "$(git show -s --format=%cs "$sha")" "$(git show -s --format=%s "$sha")"
done
echo

echo "== 2. Security entries the upstream changelogs gained in the range"
security_lines() {
    git show "$1:$2" 2>/dev/null | awk '
        /^### / { in_security = ($0 ~ /^### Security/) ; next }
        /^## /  { in_security = 0 ; next }
        in_security && NF { print }
    ' | sort -u
}
for changelog in CHANGELOG.md android/CHANGELOG.md ios/CHANGELOG.md; do
    new="$(comm -13 <(security_lines "$FROM_SHA" "$changelog") <(security_lines "$TO_SHA" "$changelog"))"
    if [[ -n "$new" ]]; then
        echo "-- $changelog"
        echo "$new"
    fi
done
echo "   (find the commit behind an entry with: git log $FROM_SHA..$TO_SHA -S'<distinctive words>')"
echo

echo "== 3. Commits OUTSIDE the watch list whose message matches: $KEYWORDS"
excludes=()
for path in "${WATCH[@]}"; do excludes+=(":(exclude)$path"); done
git log --no-merges --reverse --format='%h %cs %s' --abbrev=10 -i -E --grep="$KEYWORDS" \
    "$FROM_SHA..$TO_SHA" -- . "${excludes[@]}" \
    | grep -viE ' (update|add .* to) changelog| changelog for ' || true
echo

echo "== 4. Advisories named in the range"
git log --no-merges --format='%h %B' --abbrev=10 "$FROM_SHA..$TO_SHA" \
    | grep -oE '^[0-9a-f]{10}|GHSA-[a-z0-9]{4}-[a-z0-9]{4}-[a-z0-9]{4}|CVE-[0-9]{4}-[0-9]+' \
    | awk '/^[0-9a-f]{10}$/ { sha = $0; next } { print sha, $0 }' | sort -u -k2
echo
echo "Record every verdict in docs/UPSTREAM-SYNC-LOG.md and move .upstream-reviewed to ${TO_SHA}."
