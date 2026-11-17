#!/usr/bin/env bash
# Prints, as KEY=VALUE lines for $GITHUB_ENV or `eval`, where a release channel's
# update manifests are served and what its artifacts are called.
#
#   bash ci/update-channel-env.sh <prod|beta>
#
# WARREN_METADATA_BASE   the update host path the clients of the channel poll
# WARREN_ARTIFACT_PREFIX the installer name prefix of the channel
# WARREN_TAG_SERIES      an extended regex matching the channel's release tags
set -euo pipefail

case "${1:-}" in
prod)
	echo "WARREN_METADATA_BASE=https://api.warrenbrowse.com/updates/desktop"
	echo "WARREN_ARTIFACT_PREFIX=WarrenVPN"
	echo "WARREN_TAG_SERIES=^v[0-9]"
	;;
beta)
	echo "WARREN_METADATA_BASE=https://api.beta.warrenbrowse.com/updates/desktop"
	echo "WARREN_ARTIFACT_PREFIX=WarrenVPN-Beta"
	echo "WARREN_TAG_SERIES=^beta-v[0-9]"
	;;
*)
	echo "::error::unknown release channel '${1:-}'" >&2
	exit 1
	;;
esac
