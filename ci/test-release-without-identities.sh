#!/usr/bin/env bash
# The release guards with the secrets the repository actually holds: no macOS,
# Windows, iOS or Android signing identity, the shared update key only. GitHub
# hands an absent secret to a step as an empty string, so that is the shape
# every case below uses. A beta tag must build and publish as it always has,
# unsigned or on the debug key; a production tag must be refused.
#
#   bash ci/test-release-without-identities.sh
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

failures=0
checks=0
check() { # check <description> <expected> <actual>
	checks=$((checks + 1))
	if [ "$2" = "$3" ]; then
		printf '  ok   %s\n' "$1"
	else
		printf '  FAIL %s (expected %s, got %s)\n' "$1" "$2" "$3"
		failures=$((failures + 1))
	fi
}
status() { "$@" > /dev/null 2>&1; echo $?; }
contains() { # contains <needle> <command...>: yes when the output carries it
	local needle="$1" out
	shift
	out="$("$@" 2>&1)"
	case "$out" in *"$needle"*) echo yes ;; *) echo no ;; esac
}

# The secrets release.yml reads, as GitHub passes them today.
today=(
	WARREN_CSC_LINK_MACOS= WARREN_CSC_KEY_PASSWORD_MACOS= WARREN_NOTARIZE_KEYCHAIN_PROFILE=
	AZURE_TENANT_ID= AZURE_CLIENT_ID= AZURE_CLIENT_SECRET=
	TRUSTED_SIGNING_ENDPOINT= TRUSTED_SIGNING_ACCOUNT= TRUSTED_SIGNING_PROFILE=
	ANDROID_KEYSTORE_BASE64= WARREN_UPDATE_SIGNING_KEY=present
	IOS_DISTRIBUTION_CERT_B64= IOS_DISTRIBUTION_PASSWORD= APPLE_DEVELOPER_TEAM_ID=
	IOS_APP_PROFILE_PROD_B64= IOS_TUNNEL_PROFILE_PROD_B64=
	IOS_APP_PROFILE_BETA_B64= IOS_TUNNEL_PROFILE_BETA_B64=
)
today() { env "${today[@]}" "$@"; }

echo "signing identities"
check "a beta tag passes with today's secrets" 0 \
	"$(status today bash "$HERE/require-signing-identities.sh" beta)"
check "a production tag is refused with today's secrets" 1 \
	"$(status today bash "$HERE/require-signing-identities.sh" prod)"

echo "iOS signing material"
IOS="$HERE/ios-signing-ready.sh"
ios_complete=(IOS_DISTRIBUTION_CERT_B64=cert IOS_DISTRIBUTION_PASSWORD=pw APPLE_DEVELOPER_TEAM_ID=team)
prod_profiles=(IOS_APP_PROFILE_PROD_B64=app IOS_TUNNEL_PROFILE_PROD_B64=tunnel)
beta_profiles=(IOS_APP_PROFILE_BETA_B64=app IOS_TUNNEL_PROFILE_BETA_B64=tunnel)
check "a beta tag without any iOS material is not an error" 0 \
	"$(status today bash "$IOS" beta)"
check "it skips the iOS build" ready=false \
	"$(today bash "$IOS" beta 2> /dev/null)"
check "and says why in a notice" yes \
	"$(contains '::notice::' today bash "$IOS" beta)"
# The production profiles carry the production bundle ids: a beta build signed
# with them fails at export, so they never stand in for the beta ones.
check "a beta tag with only the production profiles skips" ready=false \
	"$(today env "${ios_complete[@]}" "${prod_profiles[@]}" bash "$IOS" beta 2> /dev/null)"
check "a beta tag missing one beta profile skips" ready=false \
	"$(today env "${ios_complete[@]}" IOS_APP_PROFILE_BETA_B64=app bash "$IOS" beta 2> /dev/null)"
check "a beta tag with its whole set signs" ready=true \
	"$(today env "${ios_complete[@]}" "${beta_profiles[@]}" bash "$IOS" beta 2> /dev/null)"
check "a production tag without iOS material is refused" 1 \
	"$(status today bash "$IOS" prod)"
check "a production tag with only the beta profiles is refused" 1 \
	"$(status today env "${ios_complete[@]}" "${beta_profiles[@]}" bash "$IOS" prod)"
check "a production tag with its whole set signs" ready=true \
	"$(today env "${ios_complete[@]}" "${prod_profiles[@]}" bash "$IOS" prod 2> /dev/null)"
check "an unknown channel is refused" 1 \
	"$(status today env "${ios_complete[@]}" "${prod_profiles[@]}" bash "$IOS" staging)"

echo "beta Android versionCode"
CODE="$HERE/android-beta-version-code.sh"
check "the next beta packs its version" 10158 "$(bash "$CODE" 1.1.58 2> /dev/null)"
check "it is above the published beta's" yes \
	"$([ "$(bash "$CODE" 1.1.58 2> /dev/null)" -gt "$(bash "$CODE" 1.1.57 2> /dev/null)" ] && echo yes || echo no)"
check "a leading zero is read in base ten" 10809 "$(bash "$CODE" 1.08.09 2> /dev/null)"
# Two digits per field: 1.1.100 would pack to the code of 1.2.0.
check "a patch above 99 is refused" 1 "$(status bash "$CODE" 1.1.100)"
check "a minor above 99 is refused" 1 "$(status bash "$CODE" 1.100.0)"
check "a version that is not X.Y.Z is refused" 1 "$(status bash "$CODE" 1.1)"

echo "version guard, with the tag series origin holds"
repo="$(mktemp -d)"
trap 'rm -rf "$repo"' EXIT
git -C "$repo" init -q -b main
git -C "$repo" config user.name test
git -C "$repo" config user.email test@example.invalid
git -C "$repo" config core.hooksPath /dev/null
git -C "$repo" commit -q --allow-empty -m one
for t in beta-v1.1.56 beta-v1.1.57 daemon-beta-v1.1.15; do git -C "$repo" tag "$t"; done
git -C "$repo" tag beta-v1.1.58
check "the next beta is the highest of its series" 0 \
	"$(cd "$repo" && status bash "$HERE/check-release-version.sh" beta 1.1.58)"
check "re-cutting the published beta is refused" 1 \
	"$(cd "$repo" && status bash "$HERE/check-release-version.sh" beta 1.1.57)"
git -C "$repo" tag v0.0.1
check "a first production tag opens its own series" 0 \
	"$(cd "$repo" && status bash "$HERE/check-release-version.sh" prod 0.0.1)"

echo
echo "$checks checks, $failures failed"
[ "$failures" -eq 0 ]
