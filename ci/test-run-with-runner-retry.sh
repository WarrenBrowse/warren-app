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

expect() { # expect <description> <expected class> <log text> [exit code]
	checks=$((checks + 1))
	local log actual
	log="$(mktemp)"
	printf '%s\n' "$3" > "$log"
	actual="$(warren_flake_class "$log" "${4:-1}")"
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

expect "a rustc crash under Rosetta is a signature, not an exit code" plain \
	"error: rustc interrupted by SIGSEGV, printing backtrace"
expect "a build killed from outside leaves no signature to grep" plain "" 137
expect "a build the timeout wrapper fired on" plain "" 124
expect "a wedge that did manage to print something still retries" plain \
	"Computing build version..." 137
expect "the VM's OOM killer taking the cargo child leaves bash's Killed notice" plain \
	"./build.sh: line 345: 24524 Killed                  cargo build \${cargo_target_arg[@]+\"\${cargo_target_arg[@]}\"}"
expect "cargo reporting a rustc the OOM killer took" plain \
	"error: could not compile \`mullvad-daemon\` (lib)

Caused by:
  process didn't exit successfully: \`rustc --crate-name mullvad_daemon\` (signal: 9, SIGKILL: kill)"

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

# The classifier is only reached through the runner loop, and the plumbing
# between them has its own way to be wrong: the exit code handed over was the
# `if` statement's (always 0), so a silent SIGKILL classified as "no signature"
# and the arm above was dead in production while every check here passed.
echo "end to end (the wrapped command's own exit status reaches the classifier)"
run_script() { # run_script <expected substring> <description> <command...>
	checks=$((checks + 1))
	local expected="$1" description="$2" output
	shift 2
	# WARREN_RETRY_LIB is exported for the source above; the child must not
	# take the library exit.
	output="$(WARREN_RETRY_LIB=0 WARREN_RETRY_ATTEMPTS=2 bash "$SCRIPT_DIR/run-with-runner-retry.sh" "$@" 2>&1)"
	if printf '%s\n' "$output" | grep -qF -- "$expected"; then
		printf '  ok   %s\n' "$description"
	else
		printf '  FAIL %s\n       expected output containing: %s\n       got:\n%s\n' \
			"$description" "$expected" "$output"
		failures=$((failures + 1))
	fi
}
run_script "runner flake ('plain' class) on attempt 1/2" \
	"a command killed without a word is retried" sh -c 'exit 137'
run_script "runner flake ('plain' class) on attempt 1/2" \
	"a command the timeout wrapper fired on is retried" sh -c 'exit 124'
run_script "no known runner signature; failing on attempt 1" \
	"a compile error still fails at once" sh -c 'echo "error[E0308]: mismatched types"; exit 101'
checks=$((checks + 1))
if WARREN_RETRY_LIB=0 bash "$SCRIPT_DIR/run-with-runner-retry.sh" sh -c 'echo built; exit 0' > /dev/null 2>&1; then
	printf '  ok   %s\n' "a command that succeeds exits 0"
else
	printf '  FAIL %s\n' "a command that succeeds exits 0"
	failures=$((failures + 1))
fi

printf '\n%d checks, %d failure(s)\n' "$checks" "$failures"
[ "$failures" -eq 0 ]
