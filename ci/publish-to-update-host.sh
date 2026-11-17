#!/usr/bin/env bash
# Publishes a release's installers and signed manifests to the update host.
#
#   ci/publish-to-update-host.sh <release files dir> <signed manifests dir> <artifact prefix> <version>
#
# Environment: WARREN_UPDATES_SSH_KEY, WARREN_UPDATES_SSH_HOST,
# WARREN_UPDATES_SSH_USER, WARREN_CHANNEL (prod or beta).
#
# The host runs a receiver (warren-updates-receive) behind the deploy key: it
# answers `list <dir>`, extracts a ustar stream with `put <dir>`, and removes
# the installers of other versions with `prune <dir> <keep prefix>`, for the
# directories `desktop` (prod) and `desktop/beta` only. Its host key is pinned
# in ci/update-host.known_hosts: this connection carries what every client
# installs, so it never trusts a key it has not seen before.
set -euo pipefail

files="${1:?usage: $0 <release files dir> <signed dir> <artifact prefix> <version>}"
signed="${2:?}"
prefix="${3:?}"
version="${4:?}"

case "${WARREN_CHANNEL:-}" in
prod) dir=desktop ;;
beta) dir=desktop/beta ;;
*)
	echo "::error::WARREN_CHANNEL must be prod or beta, got '${WARREN_CHANNEL:-}'" >&2
	exit 1
	;;
esac

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
key="$(mktemp)"
trap 'rm -f "$key"' EXIT
chmod 600 "$key"
printf '%s\n' "$WARREN_UPDATES_SSH_KEY" > "$key"

# IdentitiesOnly: the runner's ssh-agent keys are offered before -i
# identities, and the host caps authentication attempts at 3.
ssh_opts=(-i "$key" -o IdentitiesOnly=yes
	-o StrictHostKeyChecking=yes -o "UserKnownHostsFile=$here/update-host.known_hosts"
	-o HostKeyAlias=warren-updates)
target="${WARREN_UPDATES_SSH_USER}@${WARREN_UPDATES_SSH_HOST}"
receive() { # receive <verb> <args...>
	ssh "${ssh_opts[@]}" "$target" "warren-updates-receive $*"
}

# Only the installers whose (size, name) the host lacks are sent, so an
# interrupted upload re-pays its unfinished file only. The R8 mapping stays a
# GitHub release asset: the host mirrors the download page, nothing more.
(cd "$files" && ls -ln "${prefix}-${version}"-* |
	grep -v -- '-android-mapping.txt.gz' |
	awk '{print $5, $NF}' | sort -u) > "$key.local"
receive list "$dir" | awk 'NF>=5 {print $5, $NF}' | sort > "$key.remote" || true
missing=$(comm -23 "$key.local" "$key.remote" | awk '{print $2}')
rm -f "$key.local" "$key.remote"
echo "files to stream: ${missing:-none (all present)}"

# One channel per file, four at a time: a single stream is capped at
# window/RTT, and the host refuses more than five concurrent unauthenticated
# connections from one address. `wait` per pid: macOS ships bash 3.2.
rc=0
batch=""
flush_batch() {
	local p
	for p in $batch; do wait "$p" || rc=1; done
	batch=""
}
n=0
for f in $missing SHA256SUMS SHA256SUMS.sshsig install.sh; do
	(
		cd "$files"
		size=$(wc -c < "$f" | tr -d ' ')
		start=$(date +%s)
		if tar --format=ustar -cf - "$f" | receive put "$dir"; then
			took=$(($(date +%s) - start))
			[ "$took" -gt 0 ] || took=1
			echo "  streamed $f: $((size / 1048576)) MB in ${took}s ($((size / took / 1024)) KB/s)"
		else
			echo "  FAILED to stream $f"
			exit 1
		fi
	) &
	batch="$batch $!"
	n=$((n + 1))
	if [ "$n" -ge 4 ]; then
		flush_batch
		n=0
	fi
done
flush_batch
if [ "$rc" -ne 0 ]; then
	echo "::error::at least one file failed to stream to the update host"
	exit 1
fi

# Manifests last: until they land, the previous version stays served and
# coherent with the files already on the host.
(cd "$signed" && tar --format=ustar -cf - -- *.json) | receive put "$dir"
receive prune "$dir" "${prefix}-${version}-"
receive list "$dir"
