#!/usr/bin/env bash
# Run build.sh, retrying once from a clean target on failure.
#
# Why: the macOS release build is universal. build.sh compiles the native
# (aarch64) slice, then the x86_64 slice, back to back into the SAME target/
# root. Both slices compile the shared HOST build-dependencies into
# target/release/deps, and cargo occasionally loses a race there when the second
# invocation reuses/prunes artifacts the first just wrote: a freshly-expected
# dep-info file is briefly absent and the build dies with
#   error: could not parse/generate dep info at .../deps/<crate>.d
#   No such file or directory (os error 2)
# That is a transient tooling race, not a code error (the native slice builds
# fine, tests and the other platforms pass on the same commit). A clean retry
# clears the half-written target and the second attempt succeeds. Mirrors the
# SIGSEGV-under-Rosetta retry in the fetch-warren-relays action. If the retry
# also fails, exit non-zero so a genuine build break still fails the release.
set -uo pipefail

if ./build.sh "$@"; then
  exit 0
fi

echo "::warning title=Warren build::build.sh failed (likely the transient universal-build dep-info race); cleaning target and retrying once"
cargo clean || rm -rf target
exec ./build.sh "$@"
