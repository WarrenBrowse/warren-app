#!/usr/bin/env bash
# Tests for the archive step of ci/build-headless-bundle.sh.
#
#   bash ci/test-build-headless-bundle.sh
#
# The installers extract the Linux and macOS bundles as root, and GNU tar run
# as root restores the owner each entry records. Needs python3 to read the
# archive back the same way on GNU tar and bsdtar hosts.
set -uo pipefail
export LC_ALL=C

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WARREN_BUNDLE_LIB=1
export WARREN_BUNDLE_LIB
# shellcheck source=./build-headless-bundle.sh
. "$SCRIPT_DIR/build-headless-bundle.sh"
set +e

failures=0
checks=0
ok() {
    checks=$((checks + 1))
    printf '  ok   %s\n' "$1"
}
fail() {
    checks=$((checks + 1))
    failures=$((failures + 1))
    printf '  FAIL %s\n' "$1"
}

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

name=warren-headless-beta-1.2.3-linux-x86_64
mkdir -p "$TMP/out/$name/bin" "$TMP/out/$name/resources"
printf '#!/bin/sh\n' > "$TMP/out/$name/bin/warren-daemon"
chmod 0755 "$TMP/out/$name/bin/warren-daemon"
printf 'relays\n' > "$TMP/out/$name/resources/relays.json"
chmod 0644 "$TMP/out/$name/resources/relays.json"
# Run as root, the build would record root whatever the archive step does, so
# the staged files first take an ordinary account's ids, as on the runners.
if [ "$(id -u)" -eq 0 ]; then
    chown -R 1000:1000 "$TMP/out/$name"
fi

echo "archive"
if pack_tarball "$TMP/out" "$name"; then
    ok "the bundle is archived"
else
    fail "the bundle is archived"
fi

owners="$(python3 - "$TMP/out/$name.tar.gz" << 'PY'
import sys, tarfile
with tarfile.open(sys.argv[1]) as archive:
    for m in archive.getmembers():
        print(f"{m.name} {m.uid}:{m.gid} {m.uname or '-'}:{m.gname or '-'} {m.mode:o}")
PY
)"
echo "$owners" | sed 's/^/       /'

if [ -n "$owners" ] && ! echo "$owners" | awk '$2 != "0:0" { found = 1 } END { exit !found }'; then
    ok "every entry is recorded as uid 0, gid 0"
else
    fail "every entry is recorded as uid 0, gid 0"
fi
if [ -n "$owners" ] && ! echo "$owners" | awk '$3 != "-:-" { found = 1 } END { exit !found }'; then
    ok "and under no account name, which an extracting host would map to its own ids"
else
    fail "and under no account name, which an extracting host would map to its own ids"
fi
if echo "$owners" | grep -qx "$name/bin/warren-daemon 0:0 -:- 755"; then
    ok "the modes survive"
else
    fail "the modes survive"
fi

printf '\n%d checks, %d failure(s)\n' "$checks" "$failures"
[ "$failures" -eq 0 ]
