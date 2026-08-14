#!/usr/bin/env bash
# Stage the two relay-list resources every Warren daemon reads at boot into
# build/, whatever produced the binaries.
#
#   build/relays.json         inert Mullvad-format list. Warren never uses it to
#                             set up a tunnel (the daemon fetches the live exit
#                             list from GET {api_url}/v1/exits), but the upstream
#                             Mullvad relay subsystem parses it at boot, so it
#                             must exist and be a valid relay list. Querying the
#                             legacy Mullvad `/app/v1/relays` endpoint is not an
#                             option: the Warren backend answers 404.
#
#   build/warren-relays.json  the signed Warren exit list the daemon loads via
#                             `warren_relay_list_updater::load_bootstrap`. The
#                             CI `fetch-warren-relays` action writes a freshly
#                             fetched + signature-verified copy to
#                             dist-assets/warren-relays.json before any build
#                             runs; this stages it. With no baked copy (local
#                             build, offline CI) an inert placeholder is written
#                             instead: it fails the daemon's signature pin at
#                             load and is ignored, and the daemon populates the
#                             list from its startup fetch.
#
# build.sh calls this for the packaged app; ci/build-headless-bundle.sh calls it
# for the macOS and Windows headless bundles, which are assembled from a plain
# `cargo build` and would otherwise ship neither file. Both callers have to
# stage the SAME bytes: a copy of this in each would be the kind of drift where
# one platform quietly boots without an exit list.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

mkdir -p build

cat > build/relays.json <<'RELAYS_JSON'
{
  "locations": {},
  "wireguard": {
    "port_ranges": [],
    "ipv4_gateway": "10.64.0.1",
    "ipv6_gateway": "fd00::1",
    "shadowsocks_port_ranges": [],
    "relays": []
  },
  "bridge": {
    "shadowsocks": [],
    "relays": []
  }
}
RELAYS_JSON

if [[ -f dist-assets/warren-relays.json ]]; then
    cp dist-assets/warren-relays.json build/warren-relays.json
    echo "staged baked warren-relays.json bootstrap ($(wc -c < build/warren-relays.json | tr -d ' ') bytes)"
else
    cat > build/warren-relays.json <<'WARREN_RELAYS_JSON'
{"version":4,"relays":[],"generation":0,"signed_at":0,"expires_at":0,"server_pubkey_hex":"","signature_hex":""}
WARREN_RELAYS_JSON
    echo "no baked warren-relays.json; wrote inert placeholder (daemon fetches the list at runtime)"
fi
