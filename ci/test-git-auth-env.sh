#!/usr/bin/env bash
# Tests for ci/git-auth-env.sh: the job's git credential lives in the job
# environment only, reaches git exactly once per request, and never lands in a
# config file the runner keeps after the job.
#
#   bash ci/test-git-auth-env.sh
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SCRIPT="$SCRIPT_DIR/git-auth-env.sh"
TOKEN="test-token-0123456789"

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

# Runs the script in a fresh home and a fresh env file, and loads that env file
# into the current shell the way the runner does for the job's later steps.
run_script() { # run_script <home> <env file> <base url>
	HOME="$1" ENGINE_TOKEN="$TOKEN" bash "$SCRIPT" "$2" "$3" > "$1/stdout" 2>&1
}
load_env() { # load_env <env file>
	local line
	while IFS= read -r line; do
		export "${line?}"
	done < "$1"
}

echo "nothing token-bearing is written to the home directory"
home="$(mktemp -d)"
env_file="$home/github_env"
run_script "$home" "$env_file" "https://github.com/WarrenBrowse/"
check "the script succeeds" 0 "$?"
grep -rqF "$TOKEN" "$home" --exclude=github_env --exclude=stdout
check "no file under HOME holds the token" 1 "$?"
auth="$(printf 'x-access-token:%s' "$TOKEN" | base64 | tr -d '\n')"
grep -rqF "$auth" "$home" --exclude=github_env --exclude=stdout
check "no file under HOME holds the encoded header" 1 "$?"
grep -qxF "::add-mask::${auth}" "$home/stdout"
check "the encoded header is masked in the job log" 0 "$?"
grep -q '^CARGO_NET_GIT_FETCH_WITH_CLI=true$' "$env_file"
check "cargo is told to fetch with the git CLI" 0 "$?"
rm -rf "$home"

echo "the ssh form of the org's URLs is rewritten to https for this job"
home="$(mktemp -d)"
env_file="$home/github_env"
run_script "$home" "$env_file" "https://github.com/WarrenBrowse/"
rewritten="$(
	load_env "$env_file"
	HOME="$home" git ls-remote --get-url ssh://git@github.com/WarrenBrowse/warren-quinn.git
)"
check "an ssh:// git-dep resolves to https" \
	"https://github.com/WarrenBrowse/warren-quinn.git" "$rewritten"
rm -rf "$home"

echo "a header left in ~/.gitconfig by an older job is not sent twice"
if ! command -v python3 > /dev/null 2>&1; then
	echo "  FAIL python3 is required for the header count"
	exit 1
fi
work="$(mktemp -d)"
cat > "$work/server.py" << 'PY'
import http.server, sys
log = open(sys.argv[2], "a")
class H(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        for value in self.headers.get_all("Authorization") or []:
            log.write(value + "\n")
        log.write("--\n")
        log.flush()
        self.send_response(404)
        self.end_headers()
    def log_message(self, *args):
        pass
srv = http.server.HTTPServer(("127.0.0.1", 0), H)
open(sys.argv[1], "w").write(str(srv.server_address[1]))
srv.serve_forever()
PY
python3 -I "$work/server.py" "$work/port" "$work/headers" &
server=$!
for _ in $(seq 50); do [ -s "$work/port" ] && break; sleep 0.1; done
port="$(cat "$work/port")"
base="http://127.0.0.1:${port}/WarrenBrowse/"
home="$work/home"
mkdir -p "$home"
HOME="$home" git config --global "http.${base}.extraheader" "AUTHORIZATION: basic c3RhbGU="
run_script "$home" "$work/github_env" "$base"
(
	load_env "$work/github_env"
	HOME="$home" GIT_TERMINAL_PROMPT=0 git ls-remote "${base}repo.git" > /dev/null 2>&1
)
kill "$server" 2> /dev/null
wait "$server" 2> /dev/null
first_request="$(sed '/^--$/q' "$work/headers" | grep -v '^--$')"
check "one Authorization header on the first request" 1 "$(printf '%s\n' "$first_request" | grep -c .)"
check "and it is this job's" "basic ${auth}" "$first_request"
rm -rf "$work"

echo "a runner whose ~/.gitconfig still holds the credential is refused"
home="$(mktemp -d)"
HOME="$home" git config --global "url.https://x-access-token:${TOKEN}@github.com/WarrenBrowse/.insteadOf" "https://github.com/WarrenBrowse/"
run_script "$home" "$home/github_env" "https://github.com/WarrenBrowse/"
check "the script fails" 1 "$?"
grep -q '^::error::' "$home/stdout"
check "and says so in the job log" 0 "$?"
rm -rf "$home"

echo "a missing token or env file is refused"
home="$(mktemp -d)"
HOME="$home" ENGINE_TOKEN="" bash "$SCRIPT" "$home/github_env" > /dev/null 2>&1
check "an empty token fails" 1 "$?"
HOME="$home" ENGINE_TOKEN="$TOKEN" bash "$SCRIPT" > /dev/null 2>&1
check "a missing env file argument fails" 1 "$?"
rm -rf "$home"

echo
echo "$checks checks, $failures failed"
[ "$failures" = 0 ]
