#!/usr/bin/env bash
# The cost of the flow owner lookup of "Country per app" on the packet path
# (docs/app-routing.md section 2.1, Android row).
#
# Precondition: the release-shaped beta app is connected on emulator-5554 and
# at least one app has a country, so the router attributes every new flow.
#
# The flows come from `adb shell` (uid 2000, a system uid): the platform is
# asked about each of them exactly as about an app's, and the router then
# sends them through the main session, so the burst reaches no route and
# costs no exit anything. Meanwhile an ICMP echo every 200 ms (echoes are
# never looked up) crosses the same uplink pump and measures how long a
# packet of a known flow waits behind the lookups.
#
# Usage: owner-lookup.sh [flows per burst] [bursts]
# Prints the ping round trips idle and under the bursts, and the engine's own
# lookup times (`App routing: N owner lookups: ...`, logged every 500).
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

FLOWS="${1:-500}"
BURSTS="${2:-3}"
# TEST-NET-3: never answered, and never a real host's traffic.
SINK="203.0.113.9"
PING_TARGET="${PING_TARGET:-1.1.1.1}"

rtts() {
    # ping's per-reply times, in ms, one per line.
    tr -d '\r' | sed -n 's/.*time=\([0-9.]*\) ms.*/\1/p'
}

summary() {
    python3 -c '
import sys
v = sorted(float(x) for x in sys.stdin if x.strip())
if not v:
    print("no reply"); sys.exit()
q = lambda p: v[min(len(v) - 1, int(p * len(v)))]
print(f"{len(v)} replies: p50 {q(0.5):.1f} ms, p90 {q(0.9):.1f} ms, max {v[-1]:.1f} ms")
'
}

dsh logcat -c
echo "idle: $(dsh "ping -i 0.2 -c 50 $PING_TARGET" | rtts | summary)"

for burst in $(seq 1 "$BURSTS"); do
    dsh "ping -i 0.2 -c 60 $PING_TARGET" > "$OUT_DIR/owner-lookup-ping-$burst.txt" &
    pinger=$!
    sleep 2
    started="$(now_ms)"
    # One UDP socket per process, so one new flow each; all started at once.
    # toybox nc keeps a UDP socket open after its input ends: -q 1 makes
    # each one exit a second later, since thousands left running exhaust the device's
    # memory and the low memory killer takes the VPN app with them.
    dsh "i=0; while [ \$i -lt $FLOWS ]; do (echo x | nc -u -n -q 1 $SINK 9 >/dev/null 2>&1 &); i=\$((i+1)); done"
    spawned="$(now_ms)"
    wait "$pinger"
    dsh "pkill -x nc" || true
    echo "burst $burst: $FLOWS flows spawned in $((spawned - started)) ms;" \
        "ping $(rtts < "$OUT_DIR/owner-lookup-ping-$burst.txt" | summary)"
done

echo "engine:"
dsh logcat -d -v epoch | tr -d '\r' | grep "owner lookups" || echo "  no lookup line (fewer than 500 lookups?)"
