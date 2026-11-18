#!/usr/bin/env bash
# Refuses work-tracking markers in source files: lot and phase numbers, finding
# ids such as (H-1), "POC", audit tags, and references to numbered documents
# of the private backend repository. This repository is public: a reader
# cannot open those documents, and a lot number means nothing once the lot is
# closed. A comment says why the code is the way it is, in its own words.
#
#   bash ci/check-comment-markers.sh [pathspec...]           tracked files
#   bash ci/check-comment-markers.sh --staged [pathspec...]  lines a commit adds
#
# Without a pathspec, every source file, which is what CI scans; the commit
# hook holds the lines a commit adds to it.
set -euo pipefail

pattern='(^|[^A-Za-z])Lot [0-9]|(^|[^A-Za-z])Phase #?[0-9]|\([HML]-[0-9]+\)|(^|[^A-Za-z])POC([^A-Za-z]|$)|warren-core doc [0-9]|(^|[^A-Za-z])[Dd]ocs?[ /][0-9]{2,3}([^0-9]|$)|AUDIT-[0-9]{4}'
sources=('*.rs' '*.kt' '*.kts' '*.swift' '*.ts' '*.tsx' '*.toml' '*.proto')

staged=false
if [ "${1:-}" = "--staged" ]; then
	staged=true
	shift
fi
if [ "$#" -gt 0 ]; then
	sources=("$@")
fi

if "$staged"; then
	found="$(git diff --cached -U0 --no-color -- "${sources[@]}" |
		grep -E '^\+[^+]' | grep -E -- "$pattern" || true)"
else
	found="$(git grep -n -E -e "$pattern" -- "${sources[@]}" || true)"
fi

if [ -n "$found" ]; then
	echo "Work-tracking markers in source (say why in plain words instead):" >&2
	printf '%s\n' "$found" >&2
	exit 1
fi
