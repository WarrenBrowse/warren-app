#!/bin/sh
# warren-app commit gate, invoked by the shared pre-commit hook: refuses a
# commit that adds a work-tracking marker to a source file (the lines it
# leaves untouched are not its concern), or that stages a Rust file with a
# log call formatting identity material.
set -eu
bash ci/check-comment-markers.sh --staged
staged_rust="$(git diff --cached --name-only --diff-filter=ACMR -- '*.rs')"
if [ -n "$staged_rust" ]; then
	# shellcheck disable=SC2086 # one word per staged path; Rust paths here hold no spaces
	bash ci/check-no-log-identity.sh $staged_rust
fi
