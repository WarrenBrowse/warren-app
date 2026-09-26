#!/usr/bin/env bash
#
# The app consumes the quinn fork as the `warren-quinn` git-dep via
# [patch.crates-io]. If the lock re-resolves to upstream quinn (cargo only
# enforces --locked on signed builds), the patch is silently dropped and the
# app ships WITHOUT the Warren GSO/obfuscation patches. Fail loudly instead.
set -euo pipefail
if ! grep -Eq 'warren-quinn' Cargo.lock; then
    echo "::error::Cargo.lock does not pin the warren-quinn fork; the [patch.crates-io] git-dep is not locked. Regenerate Cargo.lock with scripts/dev/regen-lockfile.sh and commit it."
    exit 1
fi
if ! cargo metadata --locked --format-version=1 > /dev/null; then
    echo "::error::Cargo.lock is out of sync with the workspace; regenerate it with scripts/dev/regen-lockfile.sh and commit it."
    exit 1
fi
echo "coherence OK: Cargo.lock pins the Warren quinn fork."
