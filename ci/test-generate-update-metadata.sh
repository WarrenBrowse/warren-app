#!/usr/bin/env bash
# Tests for ci/generate-update-metadata.sh, the single place where a release and
# a refresh turn a channel into the generator's arguments: which host, which
# artifact prefix, and which forced-update floor reach the manifests the update
# key signs next.
#
#   bash ci/test-generate-update-metadata.sh
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SCRIPT="$HERE/generate-update-metadata.sh"

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

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# Stands in for ci/build-version-metadata.py: records its argument vector, one
# argument per line, so a test reads exactly what the generator would receive.
cat > "$work/generator" <<'EOF'
#!/usr/bin/env bash
printf '%s\n' "$@" > "$WARREN_TEST_ARGV"
EOF
chmod +x "$work/generator"

# run <channel> [drop versions...]: the generator's argv, one per line.
run() {
	local channel="$1"
	shift
	rm -f "$work/argv"
	WARREN_TEST_ARGV="$work/argv" \
		WARREN_METADATA_GENERATOR="$work/generator" \
		WARREN_METADATA_VERIFIER=/bin/true \
		GITHUB_REPOSITORY=WarrenBrowse/warren-app \
		bash "$SCRIPT" "$channel" "tag-x" "1.2.3" "$work/files" "$work/out" "$@" \
		> /dev/null 2>&1
	local rc=$?
	[ -f "$work/argv" ] && cat "$work/argv"
	return $rc
}
# value_of <flag> <argv>: the argument following <flag>, or "absent".
value_of() {
	printf '%s\n' "$2" | awk -v flag="$1" 'found { print; exit } $0 == flag { found = 1 } END { if (!found) print "absent" }'
}
count_of() { # count_of <flag> <argv>
	printf '%s\n' "$2" | grep -cxF -- "$1"
}
floors() {
	WARREN_UPDATE_MIN_VERSION=2.0.0 WARREN_UPDATE_MIN_VERSION_BETA=1.0.0 \
		WARREN_UPDATE_MIN_VERSION_IOS=2026.3 "$@"
}

beta="$(floors run beta)"
check "beta: the generator runs" 0 $?
check "beta: the beta floor reaches the manifests" 1.0.0 "$(value_of --min-version "$beta")"
check "beta: one floor only, never the prod one" 1 "$(count_of --min-version "$beta")"
check "beta: no iOS floor (iOS ships on prod only)" absent "$(value_of --ios-min-version "$beta")"
check "beta: the beta update host" https://api.beta.warrenbrowse.com/updates/desktop \
	"$(value_of --metadata-base-url "$beta")"
check "beta: installers served beside the manifests" https://api.beta.warrenbrowse.com/updates/desktop \
	"$(value_of --asset-base-url "$beta")"
check "beta: the beta artifacts" WarrenVPN-Beta "$(value_of --artifact-prefix "$beta")"
check "beta: the channel is named" beta "$(value_of --channel "$beta")"
check "beta: the verifier checks the published history" /bin/true "$(value_of --verifier "$beta")"

prod="$(floors run prod)"
check "prod: the prod floor" 2.0.0 "$(value_of --min-version "$prod")"
check "prod: the iOS floor" 2026.3 "$(value_of --ios-min-version "$prod")"
check "prod: the prod update host" https://api.warrenbrowse.com/updates/desktop \
	"$(value_of --metadata-base-url "$prod")"
check "prod: the prod artifacts" WarrenVPN "$(value_of --artifact-prefix "$prod")"

bare="$(run beta)"
check "no floor configured: no --min-version" absent "$(value_of --min-version "$bare")"

dropped="$(run beta ' 1.1.5' '' '1.1.6 ')"
check "each withdrawn version is passed once" 2 "$(count_of --drop-version "$dropped")"
check "a withdrawn version is trimmed" 1.1.5 "$(value_of --drop-version "$dropped")"

run staging > /dev/null
check "an unknown channel is refused" 1 $?

echo
echo "$checks checks, $failures failed"
[ "$failures" -eq 0 ]
