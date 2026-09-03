#!/usr/bin/env bash
# Runs the whole Android performance baseline matrix (docs/PERF-BASELINE.md)
# against the release-shaped beta app installed on emulator-5554, three
# iterations per scenario, then prints the median and the worst per metric.
#
#   bash android/scripts/perf/run-baseline.sh            # everything
#   bash android/scripts/perf/run-baseline.sh S2 S6      # a subset
#
# Preconditions: the app is installed and a wallet is logged in, the home
# screen is reachable, the pinned exit is Amsterdam (S4 alternates it with
# Helsinki and puts it back). Results land in $OUT_DIR (see common.sh).
set -u
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$here/common.sh"
scenarios=("$@")
[ ${#scenarios[@]} -gt 0 ] || scenarios=(S1 S2 S6 S5 S9 S8 S4 S10 S7 S7b)
rm -f "$OUT_DIR/results.tsv"
for sc in "${scenarios[@]}"; do
    case "$sc" in
    S4)
        TARGET_CITY=Helsinki bash "$here/scenarios.sh" S4 1
        TARGET_CITY=Amsterdam bash "$here/scenarios.sh" S4 2
        TARGET_CITY=Helsinki bash "$here/scenarios.sh" S4 3
        # Untimed fourth switch so the pin ends where it started.
        TARGET_CITY=Amsterdam bash "$here/scenarios.sh" S4 4
        ;;
    S7b) bash "$here/scenarios.sh" S7b 1 ;;
    *) for i in 1 2 3; do bash "$here/scenarios.sh" "$sc" "$i"; done ;;
    esac
done
adbs shell 'svc wifi enable; svc data enable' >/dev/null 2>&1
echo
python3 "$here/summarize.py" "$OUT_DIR/results.tsv" | grep -v -E 'tap_before|tap_after|gpu_percentile'
