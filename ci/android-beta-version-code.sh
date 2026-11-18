#!/usr/bin/env bash
# Prints the Android versionCode of a beta release.
#
#   bash ci/android-beta-version-code.sh <X.Y.Z>
#
# The shared derivation (mullvad-version, YYVVXZZZ) has no room for the semver
# patch, so every beta of one minor would carry the same code. The beta app has
# its own applicationId and ships by direct APK, so its code only has to grow
# along its own version line: major*10000 + minor*100 + patch. Android refuses
# a versionCode downgrade, so a field that needs a third digit is refused
# rather than packed into its neighbour (1.1.100 would read as 1.2.0).
set -euo pipefail

version="${1:-}"
if ! printf '%s' "$version" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$'; then
	echo "::error::'$version' is not an X.Y.Z beta version" >&2
	exit 1
fi
IFS=. read -r major minor patch <<< "$version"
if [ "$((10#$minor))" -gt 99 ] || [ "$((10#$patch))" -gt 99 ]; then
	echo "::error::$version does not fit the beta versionCode: minor and patch stop at 99" >&2
	exit 1
fi
echo "$((10#$major * 10000 + 10#$minor * 100 + 10#$patch))"
