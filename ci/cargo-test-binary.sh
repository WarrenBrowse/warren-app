#!/usr/bin/env bash
# Builds the library test binary of a package and prints its path, so a test
# that needs root can run as root without cargo writing root-owned files into
# the target directory.
#
#   ci/cargo-test-binary.sh <package> [cargo test options...]
#
# Pass the features of the build that came before (nextest's), or cargo builds
# the whole dependency tree a second time.
set -euo pipefail

package="${1:?usage: $0 <package> [cargo test options...]}"
shift
lib="${package//-/_}"

cargo test --locked -p "$package" --lib --no-run --message-format=json "$@" |
	python3 -c '
import json, sys
lib = sys.argv[1]
paths = []
for line in sys.stdin:
    try:
        message = json.loads(line)
    except ValueError:
        continue
    if message.get("reason") == "compiler-artifact" and message.get("executable") \
            and message["target"]["name"] == lib:
        paths.append(message["executable"])
if len(paths) != 1:
    sys.exit(f"expected one test binary for {lib}, found {paths}")
print(paths[0])
' "$lib"
