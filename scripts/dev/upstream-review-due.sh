#!/usr/bin/env bash
# Says whether an upstream review is due (docs/UPSTREAM-DETACH.md, cadence),
# from a report of scripts/dev/upstream-triage.sh and the time of the last
# review. Prints one reason per line, and nothing when no review is due.
#
# Usage: upstream-review-due.sh REPORT LAST_REVIEW_EPOCH [NOW_EPOCH]
#
# Due when the upstream changelogs gained a `### Security` entry since the
# last review (section 2 of the report), or when the last review is more
# than MAX_AGE_DAYS old. A report that cannot be read is an error, so the
# scheduled watch fails loudly instead of reporting a quiet week.
set -euo pipefail

MAX_AGE_DAYS=30

report="$1"
last_review="$2"
now="${3:-$(date +%s)}"

if ! grep -q '^== 2\. ' "$report" || ! grep -q '^== 3\. ' "$report"; then
    echo "error: $report is not a complete upstream-triage.sh report" >&2
    exit 2
fi

security="$(awk '
    /^== 2\. / { in_section = 1; next }
    /^== 3\. / { in_section = 0 }
    in_section && NF && !/^ *\(find the commit/ { print }
' "$report")"

if [[ -n "$security" ]]; then
    echo "The upstream changelogs gained Security entries since the last review:"
    echo "$security"
fi

age_days=$(((now - last_review) / 86400))
if ((age_days > MAX_AGE_DAYS)); then
    echo "The last upstream review is $age_days days old (the cadence allows $MAX_AGE_DAYS)."
fi
