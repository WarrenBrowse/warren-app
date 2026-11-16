#!/usr/bin/env bash
# Authenticates the rest of a CI job's git requests to the WarrenBrowse org,
# without writing the credential to any config file.
#
#   ENGINE_TOKEN=<token> ci/git-auth-env.sh "$GITHUB_ENV" [base url]
#
# cargo fetches the quinn and pfctl forks with the git CLI, and the first
# request of a fetch must carry the credential: GitHub's per-IP throttle of
# anonymous git answers an unauthenticated first request with a fatal error
# rather than a challenge, and the runners egress through one shared address.
#
# The header reaches the job's later steps through GIT_CONFIG_PARAMETERS, the
# environment form of `git -c` (git 2.30 on the Linux image predates
# GIT_CONFIG_COUNT and GIT_CONFIG_GLOBAL), so it ends with the job. The
# self-hosted runners keep their home directory between jobs and share it
# between runner processes, so anything written to ~/.gitconfig outlives the
# job and is readable by whatever runs there next.
set -euo pipefail

env_file="${1:-}"
base="${2:-https://github.com/WarrenBrowse/}"

if [ -z "$env_file" ]; then
	echo "usage: ENGINE_TOKEN=<token> $0 <env file> [base url]" >&2
	exit 1
fi
if [ -z "${ENGINE_TOKEN:-}" ]; then
	echo "ENGINE_TOKEN is empty" >&2
	exit 1
fi

auth="$(printf 'x-access-token:%s' "$ENGINE_TOKEN" | base64 | tr -d '\n')"
# The runner masks the secret itself, not its encoded form, and it prints a
# step's environment in the log header of every later step.
echo "::add-mask::${auth}"

key="http.${base}.extraheader"
# The empty first value resets the header list, so a header an older job left
# in ~/.gitconfig is not sent beside this one: GitHub answers a request with
# two Authorization headers with a 400. The ssh form of the org's URLs, which
# a member's git-dep may still use, is fetched over https with the same header.
params="'${key}=' '${key}=AUTHORIZATION: basic ${auth}' 'url.${base}.insteadOf=ssh://git@github.com/WarrenBrowse/'"
{
	printf 'GIT_CONFIG_PARAMETERS=%s\n' "$params"
	printf 'CARGO_NET_GIT_FETCH_WITH_CLI=true\n'
} >> "$env_file"

global="${HOME}/.gitconfig"
if [ -f "$global" ] && { grep -qF "$ENGINE_TOKEN" "$global" || grep -qF "$auth" "$global"; }; then
	echo "::warning::${global} on this runner still holds the job credential, written by an older version of this action. Remove those entries from the runner host."
fi
