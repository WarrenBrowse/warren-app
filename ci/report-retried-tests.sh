#!/usr/bin/env bash
# Name every test that nextest passed only on a retry, as a GitHub warning.
#
#   ci/report-retried-tests.sh <junit.xml>
#
# The warren-ci profile retries a failed test once, so a test that fails one
# run in two still leaves the job green. The retry is kept, a flaky test on
# main should not block every push, but each one is surfaced here, from the
# flakyFailure elements nextest writes into its JUnit report, instead of
# disappearing.
set -euo pipefail

junit="${1:?usage: report-retried-tests.sh <junit.xml>}"
[ -f "$junit" ] || { echo "::error::no nextest JUnit report at $junit" >&2; exit 1; }

# One testcase per line, with its suite (the crate) and whether it carries a
# flakyFailure. nextest writes each element on its own line.
retried="$(awk '
    /<testcase / {
        name = $0; sub(/.*<testcase name="/, "", name); sub(/".*/, "", name)
        class = $0; sub(/.*classname="/, "", class); sub(/".*/, "", class)
        flaky = 0
    }
    /<flakyFailure/ { flaky = 1 }
    /<\/testcase>/ { if (flaky) print class " " name; flaky = 0 }
' "$junit" | sort -u)"

if [ -z "$retried" ]; then
    echo "No test needed a retry"
    exit 0
fi
while read -r crate test; do
    echo "::warning title=Flaky test::$crate $test passed only on a retry"
done <<< "$retried"
