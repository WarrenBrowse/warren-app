#!/usr/bin/env bash
# Tests for upstream-review-due.sh: which upstream-triage.sh reports and review
# ages make the scheduled watch open an issue.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
DUE="$HERE/upstream-review-due.sh"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

DAY=86400
NOW=1800000000
failures=0

report() {
    # $1: the lines of section 2, or nothing.
    cat <<EOF
Upstream range: 0d06537e5f (2026-09-28) .. d84a4d947b (2026-11-16)
Commits in range: 747 (merges excluded)

== 1. Commits touching the watch list (scripts/dev/upstream-watch-paths.txt)
94a16399fc CLEAN    2026-11-13 Fix parsing of eslogger v12 messages

== 2. Security entries the upstream changelogs gained in the range
$1
   (find the commit behind an entry with: git log a..b -S'<distinctive words>')

== 3. Commits OUTSIDE the watch list whose message matches: GHSA-|CVE-|secur
7b664c0268 2026-10-15 Bump rustlts to 0.23.46 to patch new vulnerabilities

== 4. Advisories named in the range
dad8f5c661 GHSA-qwgh-2vcv-g2f7
EOF
}

check() {
    local name="$1" expected="$2" actual="$3"
    if [[ "$actual" == *"$expected"* ]]; then
        echo "ok   $name"
    else
        echo "FAIL $name"
        echo "     expected to contain: $expected"
        echo "     got: $actual"
        failures=$((failures + 1))
    fi
}

check_empty() {
    local name="$1" actual="$2"
    if [[ -z "$actual" ]]; then
        echo "ok   $name"
    else
        echo "FAIL $name"
        echo "     expected no output, got: $actual"
        failures=$((failures + 1))
    fi
}

report "" > "$WORK/quiet.txt"
check_empty "a recent review with no new Security entry is not due" \
    "$(bash "$DUE" "$WORK/quiet.txt" $((NOW - 3 * DAY)) "$NOW")"

report "-- CHANGELOG.md
- Only run launch daemon if binary \`cdhash\` matches the expected one (GHSA-f33p-7hj4-mvwj)." \
    > "$WORK/security.txt"
out="$(bash "$DUE" "$WORK/security.txt" $((NOW - 1 * DAY)) "$NOW")"
check "a new Security entry makes a review due, however recent the last one" \
    "GHSA-f33p-7hj4-mvwj" "$out"
check "the reason names the changelog the entry is in" "-- CHANGELOG.md" "$out"

check "a review older than 30 days is due" "31 days" \
    "$(bash "$DUE" "$WORK/quiet.txt" $((NOW - 31 * DAY)) "$NOW")"

check_empty "a review exactly 30 days old is not yet due" \
    "$(bash "$DUE" "$WORK/quiet.txt" $((NOW - 30 * DAY)) "$NOW")"

check_empty "advisories outside the changelogs do not open an issue by themselves" \
    "$(bash "$DUE" "$WORK/quiet.txt" $((NOW - 1 * DAY)) "$NOW")"

if bash "$DUE" "$WORK/missing.txt" "$NOW" "$NOW" >/dev/null 2>&1; then
    echo "FAIL a missing report is an error, never a quiet week"
    failures=$((failures + 1))
else
    echo "ok   a missing report is an error, never a quiet week"
fi

printf 'Upstream range: a .. b\n' > "$WORK/truncated.txt"
if bash "$DUE" "$WORK/truncated.txt" "$NOW" "$NOW" >/dev/null 2>&1; then
    echo "FAIL a report without its Security section is an error"
    failures=$((failures + 1))
else
    echo "ok   a report without its Security section is an error"
fi

if ((failures > 0)); then
    echo "$failures failure(s)"
    exit 1
fi
echo "all passed"
