#!/bin/sh
# warren-app commit gate, invoked by the shared pre-commit hook: refuses a
# commit that adds a work-tracking marker to a source file (the lines it
# leaves untouched are not its concern).
set -eu
exec bash ci/check-comment-markers.sh --staged
