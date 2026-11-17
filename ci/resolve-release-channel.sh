#!/usr/bin/env bash
# Resolves the channel, the version and the API host of an app release from
# its tag, as KEY=VALUE lines for $GITHUB_OUTPUT. The tag shape is the only
# thing that picks a channel (shared/rules/50-release-channels.md).
#
#   bash ci/resolve-release-channel.sh <ref name>
#
# Anything but a release tag is refused: a branch used to fall through to prod,
# so a manual run from main built production artifacts without being a
# release of anything.
set -euo pipefail

ref="${1:-}"
case "$ref" in
beta-v[0-9]*)
	channel=beta
	api_url=https://api.beta.warrenbrowse.com
	version="${ref#beta-v}"
	;;
v[0-9]*)
	channel=prod
	api_url=https://api.warrenbrowse.com
	version="${ref#v}"
	;;
*)
	echo "::error::'$ref' is not an app release tag (v<version> or beta-v<version>); run this workflow from a release tag" >&2
	exit 1
	;;
esac

echo "channel=$channel"
echo "version=$version"
echo "api-url=$api_url"
