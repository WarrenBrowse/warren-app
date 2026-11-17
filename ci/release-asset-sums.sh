#!/usr/bin/env bash
# The release's checksum list, computed where each file was built rather than
# read back from the draft release.
#
#   bash ci/release-asset-sums.sh record <dir>
#       In a build job: appends the hashes of every file in <dir> (what the
#       job uploads to the draft) to $GITHUB_OUTPUT as the output `sums`.
#   bash ci/release-asset-sums.sh check <expected sums> <downloaded draft dir>
#       In publish-release: refuses unless the draft holds exactly the files
#       the builds recorded, byte for byte, then writes <dir>/SHA256SUMS from
#       the recorded hashes.
#
# The checksum list is what the signed update manifests, the signed list the
# Linux install script verifies and every user checking a download rely on.
# Anyone able to write this repository's releases (a token of any job of the
# release run included) can swap a draft asset between its build and its
# publication; a list computed from the draft would then vouch for the swap.
# SHA256SUMS, its signature and the torrents are added to the release after it
# is published, so a re-run finds them on it: they describe the release and
# are never compared.
set -euo pipefail
export LC_ALL=C

die() {
	echo "::error::release-asset-sums: $*" >&2
	exit 1
}

sha256() { # sha256 <file...>: "hash  name" lines
	if command -v sha256sum > /dev/null 2>&1; then
		sha256sum -- "$@"
	else
		shasum -a 256 -- "$@"
	fi
}

# The release's own files, by name, one per line: everything but what
# describes it.
built_files() { # built_files <dir>
	local f
	for f in "$1"/* "$1"/.[!.]*; do
		[ -f "$f" ] || continue
		f="${f##*/}"
		case "$f" in
		SHA256SUMS | SHA256SUMS.sshsig | *.torrent) continue ;;
		esac
		printf '%s\n' "$f"
	done | sort
}

# "hash  name" lines, with Git Bash's binary marker ("hash *name") read as the
# same name, and nothing else accepted.
normalise() { # normalise <sums file>
	sed -E 's/^([0-9a-f]{64}) \*/\1  /' "$1" | grep -E '^[0-9a-f]{64}  [^/]+$' | sort -k2 || true
}

record() {
	local dir="${1:?usage: $0 record <dir>}" names
	names="$(built_files "$dir")"
	[ -n "$names" ] || die "$dir holds no file to upload"
	local delimiter="sums_$RANDOM$RANDOM"
	{
		echo "sums<<$delimiter"
		(cd "$dir" && printf '%s\n' "$names" | while IFS= read -r f; do sha256 "$f"; done)
		echo "$delimiter"
	} >> "${GITHUB_OUTPUT:?}"
	(cd "$dir" && printf '%s\n' "$names" | while IFS= read -r f; do sha256 "$f"; done)
}

check() {
	local expected="${1:?usage: $0 check <expected sums> <dir>}" dir="${2:?}" sums listed duplicated present
	sums="$(normalise "$expected")"
	[ -n "$sums" ] || die "no build recorded a single hash"
	listed="$(printf '%s\n' "$sums" | cut -c67- | sort)"
	duplicated="$(printf '%s\n' "$listed" | uniq -d)"
	[ -z "$duplicated" ] || die "two builds recorded the same name: $duplicated"
	present="$(built_files "$dir")"
	if [ "$listed" != "$present" ]; then
		echo "::error::the draft does not hold exactly the files the build jobs produced" >&2
		diff <(printf '%s\n' "$listed") <(printf '%s\n' "$present") >&2 || true
		exit 1
	fi
	local mismatch=""
	while IFS= read -r line; do
		local want="${line%%  *}" name="${line#*  }" got
		got="$(cd "$dir" && sha256 "$name" | cut -c1-64)"
		[ "$got" = "$want" ] || mismatch="$mismatch $name"
	done <<< "$sums"
	[ -z "$mismatch" ] || die "changed since its build:$mismatch"
	printf '%s\n' "$sums" > "$dir/SHA256SUMS"
	cat "$dir/SHA256SUMS"
}

case "${1:-}" in
record)
	shift
	record "$@"
	;;
check)
	shift
	check "$@"
	;;
*) die "usage: $0 record <dir> | check <expected sums> <dir>" ;;
esac
