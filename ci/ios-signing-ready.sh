#!/usr/bin/env bash
# Decides whether the iOS release job can sign for its channel, and prints
# `ready=true` or `ready=false` for $GITHUB_OUTPUT.
#
#   bash ci/ios-signing-ready.sh <prod|beta>
#
# Reads, under the names release.yml gives them: IOS_DISTRIBUTION_CERT_B64,
# APPLE_DEVELOPER_TEAM_ID, and the app and packet tunnel profiles of each
# channel, IOS_{APP,TUNNEL}_PROFILE_{PROD,BETA}_B64. Each channel has its own
# bundle ids, so only its own two profiles count.
#
# iOS has never shipped: without its whole set a beta tag skips the iOS build
# with a notice, and the rest of the release goes on. A production tag is
# refused instead.
set -euo pipefail

channel="${1:-}"
case "$channel" in
prod) suffix=PROD ;;
beta) suffix=BETA ;;
*)
	echo "::error::unknown release channel '$channel'" >&2
	exit 1
	;;
esac

missing=()
for name in IOS_DISTRIBUTION_CERT_B64 APPLE_DEVELOPER_TEAM_ID \
	"IOS_APP_PROFILE_${suffix}_B64" "IOS_TUNNEL_PROFILE_${suffix}_B64"; do
	if [ -z "${!name:-}" ]; then
		missing+=("$name")
	fi
done

if [ "${#missing[@]}" -eq 0 ]; then
	echo "ready=true"
	exit 0
fi
if [ "$channel" = prod ]; then
	echo "::error::a production iOS build needs its signing material; missing: ${missing[*]}" >&2
	exit 1
fi
echo "::notice::no iOS build for this beta, its signing material is missing: ${missing[*]}" >&2
echo "ready=false"
