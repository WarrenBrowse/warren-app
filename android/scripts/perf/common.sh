#!/usr/bin/env bash
# Shared helpers for the Android performance baseline on emulator-5554.
# Every timestamp is taken on the DEVICE clock (ms since the epoch) so it can
# be correlated with `logcat -v epoch` lines without host/guest skew.

export ADB_SERIAL="${ADB_SERIAL:-emulator-5554}"
export PKG="${PKG:-com.warrenbrowse.vpn.beta}"
export ACT="${ACT:-com.warrenbrowse.vpn.app.MainActivity}"
export PERF_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export OUT_DIR="${OUT_DIR:-${TMPDIR:-/tmp}/warren-perf-baseline}"
mkdir -p "$OUT_DIR"

adbs() { adb -s "$ADB_SERIAL" "$@"; }
dsh() { adbs shell "$@"; }

# Device time in ms.
now_ms() { dsh 'date +%s%3N' | tr -d '\r'; }

# Dump the UI tree to a local file and print its path.
ui_dump() {
    local f="$OUT_DIR/ui-$$.xml"
    dsh 'uiautomator dump /sdcard/ui.xml >/dev/null 2>&1; cat /sdcard/ui.xml' > "$f"
    echo "$f"
}

# Print "x y" of the centre of the first node whose text or content-desc
# matches the given regex (case-insensitive). Empty when absent.
ui_find() {
    local pattern="$1" f
    f="$(ui_dump)"
    python3 - "$f" "$pattern" <<'EOF'
import re, sys, xml.etree.ElementTree as ET
f, pat = sys.argv[1], sys.argv[2]
rx = re.compile(pat, re.I)
try:
    root = ET.parse(f).getroot()
except ET.ParseError:
    sys.exit(0)
for n in root.iter('node'):
    if rx.search(n.get('text', '') or '') or rx.search(n.get('content-desc', '') or ''):
        m = re.match(r'\[(\d+),(\d+)\]\[(\d+),(\d+)\]', n.get('bounds', ''))
        if m:
            x1, y1, x2, y2 = map(int, m.groups())
            print((x1 + x2) // 2, (y1 + y2) // 2)
            break
EOF
}

# Wait until a node matching the regex exists (timeout in seconds). Prints the
# device time (ms) at which it was first seen, or "timeout".
ui_wait() {
    local pattern="$1" timeout="${2:-20}" deadline pos
    deadline=$(( $(date +%s) + timeout ))
    while [ "$(date +%s)" -lt "$deadline" ]; do
        pos="$(ui_find "$pattern")"
        if [ -n "$pos" ]; then now_ms; return 0; fi
    done
    echo timeout
    return 1
}

# Tap the node matching the regex. Prints "t_before t_after" device ms around
# the `input tap` (the command itself costs a few hundred ms of process
# start; the tap is injected shortly before t_after).
ui_tap() {
    local pattern="$1" pos
    pos="$(ui_find "$pattern")"
    [ -n "$pos" ] || { echo "no node for /$pattern/" >&2; return 1; }
    tap_xy $pos
}

tap_xy() {
    dsh "t0=\$(date +%s%3N); input tap $1 $2; t1=\$(date +%s%3N); echo \$t0 \$t1" | tr -d '\r'
}

gfx_reset() { dsh dumpsys gfxinfo "$PKG" reset >/dev/null; }
gfx_summary() {
    dsh dumpsys gfxinfo "$PKG" | sed -n '/Total frames rendered/,/Number Frame deadline missed$/p' | tr -d '\r'
}

# Logcat since a device epoch ms, filtered to the app's tags.
logcat_since() {
    local since_ms="$1"
    local sec=$(( since_ms / 1000 )) ms=$(( since_ms % 1000 ))
    adbs logcat -d -v epoch -T "$sec.$(printf '%03d' $ms)" 2>/dev/null | tr -d '\r'
}

# First epoch ms of a logcat line matching a regex, since a given time.
log_first() {
    local since_ms="$1" pattern="$2"
    logcat_since "$since_ms" | grep -E "$pattern" | head -1 | awk '{gsub(/\./,"",$1); print substr($1,1,13)}'
}

app_pid() { dsh pidof "$PKG" | tr -d '\r'; }

median3() { printf '%s\n' "$@" | sort -n | sed -n '2p'; }
max3() { printf '%s\n' "$@" | sort -n | tail -1; }
# Screen pixel polling: a raw screencap costs about 0.3 s on this AVD, against
# about 1.9 s for a uiautomator dump, so screen transitions are timed by
# watching one pixel that only the target screen paints.
# px x y -> "R G B device_ms" (device time taken right after the capture).
px() {
    adbs exec-out "screencap; echo; date +%s%3N" | python3 -c '
import sys, struct
x, y = int(sys.argv[1]), int(sys.argv[2])
data = sys.stdin.buffer.read()
w, h = struct.unpack_from("<II", data, 0)
off = 16 + (y * w + x) * 4
r, g, b = data[off], data[off + 1], data[off + 2]
tail = data[16 + w * h * 4:].decode(errors="ignore").split()
print(r, g, b, tail[-1] if tail else "")' "$1" "$2"
}
# px_wait x y "R G B" tolerance timeout_s -> device ms when the pixel first
# matched, or "timeout".
px_wait() {
    local x="$1" y="$2" want="$3" tol="${4:-24}" timeout="${5:-20}" deadline r g b t
    read -r wr wg wb <<<"$want"
    deadline=$(( $(date +%s) + timeout ))
    while [ "$(date +%s)" -lt "$deadline" ]; do
        read -r r g b t <<<"$(px "$x" "$y")"
        if [ $(( r > wr ? r - wr : wr - r )) -le "$tol" ] && [ $(( g > wg ? g - wg : wg - g )) -le "$tol" ] && [ $(( b > wb ? b - wb : wb - b )) -le "$tol" ]; then
            echo "$t"; return 0
        fi
    done
    echo timeout; return 1
}
# region_bright x1 y1 x2 y2 -> "count device_ms": number of pixels with
# luminance above 128 in the rectangle; a title or a button that exists on
# one screen only makes the count jump between screens.
region_bright() {
    adbs exec-out "screencap; echo; date +%s%3N" | python3 -c '
import sys, struct
x1, y1, x2, y2 = map(int, sys.argv[1:5])
data = sys.stdin.buffer.read()
w, h = struct.unpack_from("<II", data, 0)
n = 0
for y in range(y1, y2):
    row = 16 + y * w * 4
    for x in range(x1, x2):
        o = row + x * 4
        if (data[o] * 299 + data[o + 1] * 587 + data[o + 2] * 114) // 1000 > 128:
            n += 1
tail = data[16 + w * h * 4:].decode(errors="ignore").split()
print(n, tail[-1] if tail else "")' "$1" "$2" "$3" "$4"
}
# region_wait x1 y1 x2 y2 <lt|gt> threshold timeout_s -> device ms when the
# bright count first satisfied the comparison, or "timeout".
region_wait() {
    local x1="$1" y1="$2" x2="$3" y2="$4" op="$5" thr="$6" timeout="${7:-20}" deadline n t
    deadline=$(( $(date +%s) + timeout ))
    while [ "$(date +%s)" -lt "$deadline" ]; do
        read -r n t <<<"$(region_bright "$x1" "$y1" "$x2" "$y2")"
        if { [ "$op" = lt ] && [ "$n" -lt "$thr" ]; } || { [ "$op" = gt ] && [ "$n" -gt "$thr" ]; }; then
            echo "$t"; return 0
        fi
    done
    echo timeout; return 1
}
