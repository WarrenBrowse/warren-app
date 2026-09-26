#!/usr/bin/env bash
#
# Fetch the live signed Warren exit list from <api-url>/v1/exits, verify its
# Ed25519 signature against the pinned production server pubkey with
# warren-discovery-core's warren_relays_verify (the authoritative
# canonicalization), and write it to dist-assets/warren-relays.json so the
# packaged client ships with an up-to-date bootstrap cache.
#
#   ci/fetch-warren-relays.sh <api-url> [expected-pubkey-hex]
#
# A fetch failure (API down, offline runner) is a warning: the client ships
# without a baked list and fetches on first launch. A fetched list whose
# signature does NOT verify fails the build: an unverifiable bootstrap is
# never baked. Needs the ../warren-contract sibling checkout.
#
# There is no default API host on purpose: a default once baked the production
# network's exit list into whatever was being built, which stays invisible
# while both hosts answer and becomes a wrong-network app when they diverge.
set -uo pipefail

API="${1:-}"
PUBKEY="${2:-4c2c9253c426ae4db4cc88703f9ac802a020420c7fea6479c87af530ada72c3e}"
OUT="dist-assets/warren-relays.json"
LOG="$(mktemp)"
trap 'rm -f "$LOG"' EXIT

if [ -z "$API" ]; then
    echo "::error title=Warren bootstrap::no API host given; pass the channel's API host"
    exit 1
fi
URL="$API/v1/exits"
echo "Fetching signed Warren exit list: $URL"
if ! curl -fsS --max-time 30 "$URL" -o "$OUT"; then
    echo "::warning title=Warren bootstrap::could not fetch $URL; shipping WITHOUT a baked exit list (clients fetch on first launch)"
    rm -f "$OUT"
    exit 0
fi
echo "Verifying signature against pinned server pubkey (build-time gate)"
for attempt in 1 2 3; do
    cargo run --quiet \
        --manifest-path ../warren-contract/warren-discovery/Cargo.toml \
        -p warren-discovery-core --bin warren_relays_verify -- \
        "$OUT" --expected-pubkey "$PUBKEY" 2>&1 | tee "$LOG"
    rc=${PIPESTATUS[0]}
    if [ "$rc" -eq 0 ]; then
        echo "Baked verified warren-relays.json ($(wc -c < "$OUT" | tr -d ' ') bytes)"
        exit 0
    fi
    if grep -qE "SIGSEGV|signal: 11|signal: 4|signal: 6|Illegal instruction|internal compiler error: Segmentation" "$LOG"; then
        echo "::warning title=Warren bootstrap::verifier build crashed under Rosetta (attempt $attempt/3); retrying"
        continue
    fi
    echo "::error title=Warren bootstrap::fetched exit list FAILED signature verification; refusing to bake into the release"
    rm -f "$OUT"
    exit 1
done
echo "::error title=Warren bootstrap::verifier kept crashing under Rosetta; cannot validate the bootstrap list"
rm -f "$OUT"
exit 1
