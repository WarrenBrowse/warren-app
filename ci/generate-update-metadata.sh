#!/usr/bin/env bash
# Writes the unsigned update manifests (and the torrents, beside the
# installers) of one channel's release. The release workflow and the monthly
# refresh both call it, so the host, the artifact prefix and the forced-update
# floor a channel's manifests carry cannot differ between the two paths.
#
#   bash ci/generate-update-metadata.sh <prod|beta> <tag> <version> <release files dir> <out dir> [withdrawn version...]
#
# Environment: WARREN_METADATA_VERIFIER (the mullvad-version-metadata binary,
# whose `verify` checks the published history before it is carried forward),
# GITHUB_REPOSITORY, and the floors WARREN_UPDATE_MIN_VERSION (prod),
# WARREN_UPDATE_MIN_VERSION_BETA (beta), WARREN_UPDATE_MIN_VERSION_IOS (prod
# only, iOS ships on prod alone). WARREN_METADATA_GENERATOR replaces the
# generator in tests.
set -euo pipefail

channel="${1:?usage: $0 <prod|beta> <tag> <version> <release files dir> <out dir> [withdrawn version...]}"
tag="${2:?}"
version="${3:?}"
files="${4:?}"
out="${5:?}"
shift 5

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
eval "$(bash "$here/update-channel-env.sh" "$channel")"

args=()
case "$channel" in
prod)
	[ -z "${WARREN_UPDATE_MIN_VERSION:-}" ] || args+=(--min-version "$WARREN_UPDATE_MIN_VERSION")
	[ -z "${WARREN_UPDATE_MIN_VERSION_IOS:-}" ] || args+=(--ios-min-version "$WARREN_UPDATE_MIN_VERSION_IOS")
	;;
beta)
	# The beta series is independent of prod, so it has a floor of its own and
	# never takes the prod one.
	[ -z "${WARREN_UPDATE_MIN_VERSION_BETA:-}" ] || args+=(--min-version "$WARREN_UPDATE_MIN_VERSION_BETA")
	;;
esac

for withdrawn in "$@"; do
	withdrawn="$(printf '%s' "$withdrawn" | tr -d '[:space:]')"
	if [ -n "$withdrawn" ]; then args+=(--drop-version "$withdrawn"); fi
done

generator="${WARREN_METADATA_GENERATOR:-python3 $here/build-version-metadata.py}"
# shellcheck disable=SC2086 # the generator is a command line
$generator \
	--verifier "${WARREN_METADATA_VERIFIER:?}" \
	--tag "$tag" \
	--version "$version" \
	--release-dir "$files" \
	--repo "${GITHUB_REPOSITORY:?}" \
	--changelog "$here/../CHANGELOG.md" \
	--metadata-base-url "$WARREN_METADATA_BASE" \
	--asset-base-url "$WARREN_METADATA_BASE" \
	--artifact-prefix "$WARREN_ARTIFACT_PREFIX" \
	--channel "$channel" \
	--out-dir "$out" \
	--now "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
	${args[@]+"${args[@]}"}
