#!/usr/bin/env bash
# Tests for the failure classifier in ci/run-with-runner-retry.sh.
#
#   bash ci/test-run-with-runner-retry.sh
#
# Both directions of a misclassification are expensive, which is why they are
# pinned here rather than reviewed by eye:
#
#   too narrow  a release dies on a runner transient, and since the publish
#               gates on the Windows bundle, the whole release dies with it;
#   too broad   a genuine regression is retried until it reads as intermittent,
#               which is how a real bug gets filed as "flaky CI" and shipped.
#
# The log excerpts are the real ones, from the runs recorded in the
# `warren-ci-runners` skill.
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WARREN_RETRY_LIB=1
export WARREN_RETRY_LIB
# shellcheck source=./run-with-runner-retry.sh
. "$SCRIPT_DIR/run-with-runner-retry.sh"

failures=0
checks=0

expect() { # expect <description> <expected class> <log text>
	checks=$((checks + 1))
	local log actual
	log="$(mktemp)"
	printf '%s\n' "$3" > "$log"
	actual="$(warren_flake_class "$log")"
	rm -f "$log"
	if [ "$actual" = "$2" ]; then
		printf '  ok   %s\n' "$1"
	else
		printf '  FAIL %s\n       expected: %s\n       actual:   %s\n' \
			"$1" "${2:-<no retry>}" "${actual:-<no retry>}"
		failures=$((failures + 1))
	fi
}

echo "runner transients (must retry)"
expect "cargo could not spawn rustc" plain \
	"error: could not execute process \`C:\\rustup\\toolchains\\stable-x86_64-pc-windows-msvc\\bin\\rustc.exe --crate-name uniffi\` (never executed)

Caused by:
  The directory name is invalid. (os error 267)"
expect "the same spawn failure reported as a busy device" plain \
	"error: could not execute process \`rustc.exe\` (never executed)

Caused by:
  Device or resource busy"
expect "the transient linker failure gets a clean first" clean \
	"error: linking with \`link.exe\` failed: exit code: 1181
  = note: LINK : fatal error LNK1181: cannot open input file 'windows.0.52.0.lib'"

echo "real failures (must NOT retry)"
expect "a compile error is the code's own" "" \
	"error[E0308]: mismatched types
  --> mullvad-daemon/src/lib.rs:42:5"
expect "a failing test is not a flake" "" \
	"test result: FAILED. 3 passed; 1 failed; 0 ignored"
expect "a missing file the build really needs" "" \
	"::error::missing release binary: target/x86_64-pc-windows-msvc/release/warren.exe"
expect "an empty log retries nothing" "" ""

# The one that matters most: LNK1181 wins over a spawn signature in the same
# log, because a clean also fixes the spawn case while the reverse is not true.
echo "precedence"
expect "a log carrying both takes the clean path" clean \
	"error: could not execute process \`rustc.exe\` (never executed)
LINK : fatal error LNK1181: cannot open input file 'windows.0.52.0.lib'"

printf '\n%d checks, %d failure(s)\n' "$checks" "$failures"
[ "$failures" -eq 0 ]
