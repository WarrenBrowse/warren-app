#!/usr/bin/env bash
# Install-test the sysvinit flavour of the Warren VPN .deb inside a Debian
# container that has no systemd, which is what an MX Linux sysVinit edition
# looks like from the package's point of view.
#
# What it is guarding: the systemd .deb leaves dpkg half-configured on such a
# host (its postinst runs `systemctl enable` under `set -e`), and that failure
# only ever shows up on the user's machine. Everything here is a real dpkg
# transaction and a real daemon process, not a lint of the scripts.
#
# The package is streamed into the container with `docker cp` rather than
# bind-mounted: the CI runner is itself a container whose workspace lives in a
# named volume, so a host path from inside it does not resolve for the daemon.
#
# The container is privileged because the daemon owns the kernel firewall
# (nftables) at startup; without it the daemon is up but cannot do its job, and
# the smoke test would prove nothing.
#
# Usage: ci/test-sysvinit-deb.sh <sysvinit.deb> [systemd.deb]
#
# Passing the systemd .deb as well additionally asserts that the two flavours
# refuse to co-install.
#
# Image override: WARREN_SYSVINIT_TEST_IMAGE (default debian:bookworm-slim,
# the base of MX 23; the daemon is built against glibc 2.31 so it also runs on
# the trixie base of MX 25).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

deb="${1:?usage: test-sysvinit-deb.sh <sysvinit.deb> [systemd.deb]}"
systemd_deb="${2:-}"
image="${WARREN_SYSVINIT_TEST_IMAGE:-debian:bookworm-slim}"

command -v docker > /dev/null || {
    echo "::error::docker is required to install-test the sysvinit .deb" >&2
    exit 1
}

echo "Install-testing $(basename "$deb") on $image (amd64)"
docker pull --quiet --platform linux/amd64 "$image" > /dev/null

cid="$(docker create --privileged --platform linux/amd64 \
    --env WARREN_TEST_HAS_SYSTEMD_DEB="$([ -n "$systemd_deb" ] && echo 1 || echo 0)" \
    "$image" bash /tmp/in-container-test.sh)"
trap 'docker rm -f "$cid" > /dev/null 2>&1 || true' EXIT

docker cp "$SCRIPT_DIR/sysvinit-deb-container-test.sh" "$cid:/tmp/in-container-test.sh"
docker cp "$deb" "$cid:/tmp/warren-sysvinit.deb"
if [ -n "$systemd_deb" ]; then
    docker cp "$systemd_deb" "$cid:/tmp/warren-systemd.deb"
fi

docker start -a "$cid"
