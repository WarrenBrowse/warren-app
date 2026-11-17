#!/usr/bin/env bash
# Tests for ci/report-retried-tests.sh, which names every test nextest only
# passed on a retry, so a flaky test stops hiding behind the retry.
#
#   bash ci/test-report-retried-tests.sh
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPORT="$SCRIPT_DIR/report-retried-tests.sh"
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

# The shape nextest writes: a test that failed once and passed on its retry
# carries a flakyFailure element.
cat > "$TMP/flaky.xml" << 'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<testsuites name="nextest-run" tests="3" failures="0" errors="0">
    <testsuite name="talpid-core" tests="2" disabled="0" errors="0" failures="0">
        <testcase name="resolver::tests::binds" classname="talpid-core" timestamp="2026-11-17T10:00:00Z" time="0.2">
            <flakyFailure type="test failure" message="exit code 101" timestamp="2026-11-17T10:00:00Z" time="0.1"/>
        </testcase>
        <testcase name="firewall::tests::stable" classname="talpid-core" timestamp="2026-11-17T10:00:00Z" time="0.1"/>
    </testsuite>
    <testsuite name="mullvad-daemon" tests="1" disabled="0" errors="0" failures="0">
        <testcase name="tunnel::tests::reconnects" classname="mullvad-daemon" timestamp="2026-11-17T10:00:00Z" time="1.0">
            <flakyFailure type="test failure" message="exit code 101" timestamp="2026-11-17T10:00:00Z" time="0.5"/>
            <flakyFailure type="test failure" message="exit code 101" timestamp="2026-11-17T10:00:00Z" time="0.5"/>
        </testcase>
    </testsuite>
</testsuites>
EOF

cat > "$TMP/clean.xml" << 'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<testsuites name="nextest-run" tests="1" failures="0" errors="0">
    <testsuite name="talpid-core" tests="1" disabled="0" errors="0" failures="0">
        <testcase name="firewall::tests::stable" classname="talpid-core" timestamp="2026-11-17T10:00:00Z" time="0.1"/>
    </testsuite>
</testsuites>
EOF

bash "$REPORT" "$TMP/flaky.xml" > "$TMP/out" 2>&1
status=$?
check "a run with retried tests does not fail the job" [ "$status" -eq 0 ]
check "names the first retried test" grep -q '^::warning.*talpid-core resolver::tests::binds' "$TMP/out"
check "names the second retried test once" [ "$(grep -c 'tunnel::tests::reconnects' "$TMP/out")" -eq 1 ]
check "does not name a test that passed first time" \
	bash -c "! grep -q 'firewall::tests::stable' '$TMP/out'"

bash "$REPORT" "$TMP/clean.xml" > "$TMP/out" 2>&1
status=$?
check "a clean run passes" [ "$status" -eq 0 ]
check "a clean run raises no warning" bash -c "! grep -q '^::warning' '$TMP/out'"

bash "$REPORT" "$TMP/missing.xml" > "$TMP/out" 2>&1
status=$?
check "a missing report is an error, not a clean run" [ "$status" -ne 0 ]

printf '\n%d checks, %d failure(s)\n' "$checks" "$failures"
[ "$failures" -eq 0 ]
