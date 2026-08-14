#!/usr/bin/env bash
# Build the staged NixOS flake for real, smoke the result, and pack the tarball
# that ships to users.
#
# "Build for real" is the point. A flake that only evaluates proves nothing: the
# failure mode of packaging a prebuilt Electron app on NixOS is a missing shared
# library, and autoPatchelfHook only reports it while linking. So this runs
# `nix build`, then executes the binaries out of the store path.
#
# The .deb is pre-seeded into the store with the SAME fixed-output hash the
# flake pins. Two things follow: the build needs no network for it (the release
# it pins is not published yet at this point in the pipeline), and a hash that
# disagrees with the artifact turns into a failed download instead of a package
# nobody can install.
#
# Nix runs in a container: the CI runner has no nix, and installing one needs
# root it does not have. /nix is a named volume so the nixpkgs closure survives
# between runs (the first run pays for it, later ones do not).
#
# Usage: ci/build-nixos-flake.sh <stage-dir> <input.deb> <output.tar.gz>
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

stage="${1:?usage: build-nixos-flake.sh <stage-dir> <input.deb> <output.tar.gz>}"
deb="${2:?missing input .deb}"
output="${3:?missing output tarball}"

image="${WARREN_NIX_IMAGE:-nixos/nix:latest}"

command -v docker > /dev/null || {
    echo "::error::docker is required to build the NixOS flake" >&2
    exit 1
}

[ -f "$stage/flake.nix" ] || {
    echo "::error::$stage is not a staged flake (no flake.nix); run ci/stage-nixos-flake.sh first" >&2
    exit 1
}

# The tarball keeps a single top-level directory, which the Nix tarball fetcher
# strips: a human who extracts it gets a named directory rather than a pile of
# files in their cwd.
flake_dir_name="$(basename "$output")"
flake_dir_name="${flake_dir_name%.tar.gz}"

echo "Building the NixOS flake on $image (amd64)"
docker pull --quiet --platform linux/amd64 "$image" > /dev/null

# The /nix cache is keyed by the image it was seeded from, never by a fixed
# name. In this image /etc/group and /etc/passwd are symlinks into
# /nix/store/...-base-system/etc/, so mounting a /nix populated by an OLDER
# image hides them: the `nixbld` group vanishes and nix refuses to build any
# derivation ("the group 'nixbld' specified in 'build-users-group' does not
# exist"), which reads as a broken nix rather than a stale cache. It took the
# beta-v1.1.13 and beta-v1.1.14 releases down when `nixos/nix:latest` moved.
# Docker seeds an empty named volume from the image, so a new key is a correct
# store, at the cost of re-downloading the closure once.
image_id="$(docker image inspect --format '{{.Id}}' "$image")"
image_id="${image_id#sha256:}"
store_volume="${WARREN_NIX_STORE_VOLUME:-warren-nix-store-$(printf '%s' "$image_id" | cut -c1-12)}"

# Bound the disk: every superseded key holds a full nixpkgs closure. A volume
# another job still holds refuses to be removed, which is the wanted safety.
docker volume ls --quiet --filter 'name=warren-nix-store-' | grep -vFx "$store_volume" | while read -r stale; do
    docker volume rm "$stale" > /dev/null 2>&1 && echo "removed superseded nix store $stale" || true
done

cid="$(docker create --platform linux/amd64 \
    --volume "$store_volume:/nix" \
    --env "WARREN_FLAKE_DIR_NAME=$flake_dir_name" \
    "$image" sh /tmp/warren-nixos/in-container-build.sh)"
trap 'docker rm -f "$cid" > /dev/null 2>&1 || true' EXIT

# docker cp will not create a missing parent, so the directory is staged
# locally and copied in one piece.
local_stage="$(mktemp -d)"
trap 'docker rm -f "$cid" > /dev/null 2>&1 || true; rm -rf "$local_stage"' EXIT
mkdir -p "$local_stage/warren-nixos"
cp "$SCRIPT_DIR/nixos-flake-container-build.sh" "$local_stage/warren-nixos/in-container-build.sh"
cp -r "$stage" "$local_stage/warren-nixos/flake"
cp "$deb" "$local_stage/warren-nixos/$(basename "$deb")"
docker cp "$local_stage/warren-nixos" "$cid:/tmp/warren-nixos"

docker start -a "$cid"

mkdir -p "$(dirname "$output")"
docker cp "$cid:/tmp/warren-nixos/out/$flake_dir_name.tar.gz" "$output"
# The lock the build resolved is what the published tarball carries, so keep the
# staged copy in step for anyone inspecting it afterwards.
docker cp "$cid:/tmp/warren-nixos/flake/flake.lock" "$stage/flake.lock"
echo "Built $output ($(du -h "$output" | cut -f1))"
