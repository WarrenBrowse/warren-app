#!/usr/bin/env bash
# Run a build command, retrying only the failures that are the Windows runner
# rather than the code.
#
#   ci/run-with-runner-retry.sh <command> [args...]
#
# The release publish now gates on the headless Windows bundle, so a runner
# flake is no longer a warning on one artifact: it blocks the whole release,
# app included. That is the right coupling (a release is the whole artifact set
# or it is not a release) but it is only affordable if the transients absorb
# themselves.
#
# Two signature classes, from `warren-ci-runners`:
#
#   LNK1181            the windows-rs import lib is briefly unopenable at link
#                      time, usually under a concurrent runner. A clean target
#                      lands on a fresh state, so this one retries AFTER
#                      `cargo clean`.
#
#   spawn failures     `rustc.exe ... (never executed)` with `The directory name
#                      is invalid. (os error 267)` or `Device or resource busy`.
#                      cargo could not SPAWN the compiler, so the crate it names
#                      never had a chance to be wrong and nothing on disk is
#                      poisoned. A plain retry is the fix; cleaning would only
#                      pay for a full rebuild.
#
# Anything else fails immediately, on the first attempt: a real compile error
# must not cost three builds before it is reported. That is the difference from
# ci/build-with-retry.sh, which retries any failure once and is kept as is for
# the GUI build it wraps.
#
# The classifier is exercised by ci/test-run-with-runner-retry.sh, because
# getting it wrong in either direction is expensive: too narrow and releases die
# on infra, too broad and a genuine regression is retried until it looks
# intermittent.
set -uo pipefail

ATTEMPTS="${WARREN_RETRY_ATTEMPTS:-3}"

# Prints the retry strategy a failure log calls for: "clean", "plain", or
# nothing at all when the failure is the code's own.
warren_flake_class() { # warren_flake_class <logfile>
	if grep -q "LNK1181" "$1"; then
		echo clean
	elif grep -qE "\(never executed\)|os error 267|The directory name is invalid|Device or resource busy" "$1"; then
		echo plain
	fi
}

# Sourced by the test, which wants the classifier and not a build.
if [ "${WARREN_RETRY_LIB:-0}" = "1" ]; then
	return 0 2> /dev/null || exit 0
fi

[ "$#" -ge 1 ] || {
	echo "usage: ci/run-with-runner-retry.sh <command> [args...]" >&2
	exit 2
}

log="$(mktemp)"
trap 'rm -f "$log"' EXIT

attempt=1
while :; do
	if "$@" 2>&1 | tee "$log"; then
		exit 0
	fi

	class="$(warren_flake_class "$log")"
	if [ -z "$class" ]; then
		echo "::error title=Warren build::the failure carries no known runner signature; failing on attempt $attempt"
		exit 1
	fi
	if [ "$attempt" -ge "$ATTEMPTS" ]; then
		echo "::error title=Warren build::still failing on the '$class' runner signature after $attempt attempts"
		exit 1
	fi

	echo "::warning title=Warren build::runner flake ('$class' class) on attempt $attempt/$ATTEMPTS; retrying"
	if [ "$class" = clean ]; then
		cargo clean || true
	fi
	attempt=$((attempt + 1))
done
