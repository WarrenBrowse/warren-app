#!/usr/bin/env bash
# Android performance baseline scenarios S1..S10 on emulator-5554.
# Usage: scenarios.sh <scenario> <iteration>
# Every metric is appended to $OUT_DIR/results.tsv as: scenario<TAB>iter<TAB>metric<TAB>value
set -u
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
RES="$OUT_DIR/results.tsv"
SC="$1"; IT="$2"
record() { printf '%s\t%s\t%s\t%s\n' "$SC" "$IT" "$1" "$2" | tee -a "$RES"; }
save_gfx() { gfx_summary | tee "$OUT_DIR/$SC-$IT-gfx.txt" | grep -E "Total frames|Janky frames:|50th|90th|95th|99th" | sed 's/^ *//' | while IFS= read -r line; do record "gfx:$(echo "$line" | cut -d: -f1 | tr ' ' '_')" "$(echo "$line" | cut -d: -f2- | sed 's/^ *//')"; done; }
save_log() { logcat_since "$1" > "$OUT_DIR/$SC-$IT-logcat.txt"; }

# Poll logcat until a line matches (device ms since), print its epoch ms.
wait_log() {
    local since="$1" pattern="$2" timeout="${3:-40}" deadline t
    deadline=$(( $(date +%s) + timeout ))
    while [ "$(date +%s)" -lt "$deadline" ]; do
        t="$(log_first "$since" "$pattern")"
        if [ -n "$t" ]; then echo "$t"; return 0; fi
        sleep 0.3
    done
    echo ""; return 1
}

go_home_screen() {
    # Back out of whatever screen is open until the connection card is visible.
    local i
    for i in 1 2 3 4; do
        if [ -n "$(ui_find '^(Connect|Disconnect|Cancel)$')" ]; then return 0; fi
        dsh input keyevent KEYCODE_BACK; sleep 0.8
    done
    dsh am start -n "$PKG/$ACT" >/dev/null 2>&1; sleep 2
}

ensure_disconnected() {
    go_home_screen
    if [ -z "$(ui_find '^Connect$')" ]; then
        ui_tap '^(Disconnect|Cancel)$' >/dev/null || true
        ui_wait '^Connect$' 20 >/dev/null || true
        sleep 1
    fi
}

ensure_connected() {
    go_home_screen
    if [ -n "$(ui_find '^Connect$')" ]; then
        local t; t="$(now_ms)"
        ui_tap '^Connect$' >/dev/null
        wait_log "$t" 'multi-hop tunnel up' 60 >/dev/null || echo "WARN: tunnel did not come up" >&2
        sleep 3
    fi
}

case "$SC" in
S1)
    # Cold start: TTID from am start -W, TTFD from the Fully drawn line.
    dsh am force-stop "$PKG"; sleep 3
    t0="$(now_ms)"
    out="$(dsh am start -W -n "$PKG/$ACT" | tr -d '\r')"
    echo "$out" > "$OUT_DIR/$SC-$IT-amstart.txt"
    record ttid_TotalTime_ms "$(echo "$out" | awk -F': ' '/^TotalTime/{print $2}')"
    record WaitTime_ms "$(echo "$out" | awk -F': ' '/^WaitTime/{print $2}')"
    sleep 6
    save_log "$t0"
    disp="$(grep -E 'Displayed .*MainActivity' "$OUT_DIR/$SC-$IT-logcat.txt" | head -1 | grep -oE '\+[0-9]+(s[0-9]+)?ms' | head -1)"
    fd="$(grep -E 'Fully drawn .*MainActivity' "$OUT_DIR/$SC-$IT-logcat.txt" | head -1 | grep -oE '\+[0-9]+(s[0-9]+)?ms' | head -1)"
    record displayed "$disp"
    record fully_drawn "$fd"
    ;;
S2)
    # Connect then disconnect, timed from the tap through the engine markers.
    ensure_disconnected
    gfx_reset
    read -r tb ta <<<"$(ui_tap '^Connect$')"
    record tap_before_ms "$tb"; record tap_after_ms "$ta"
    tup="$(wait_log "$tb" 'multi-hop tunnel up' 60)"
    sleep 7   # covers the 6 s connecting zoom
    save_gfx  # this is S3: frames of the connecting window measured on the same run
    tdisp="$(log_first "$tb" 'dispatched Quinn connect intent')"
    tnat="$(log_first "$tb" 'multi-hop connect \(')"
    record dispatch_after_tap_ms $(( ${tdisp:-0} - ta ))
    record native_connect_after_tap_ms $(( ${tnat:-0} - ta ))
    record tunnel_up_after_tap_ms $(( ${tup:-0} - ta ))
    record tunnel_up_after_dispatch_ms $(( ${tup:-0} - ${tdisp:-0} ))
    sleep 3
    read -r db da <<<"$(ui_tap '^Disconnect$')"
    # The Connect button is the only white fill at that spot (241,243,241);
    # every tunnelled state paints it in a darker tone. Polled first, the
    # engine marker is read from the log afterwards.
    tui="$(px_wait 540 2097 '241 243 241' 24 20)"
    tdown="$(wait_log "$db" 'multi-hop tunnel cancelled by Kotlin|multi-hop session ended' 30)"
    record disconnect_engine_after_tap_ms $(( ${tdown:-0} - da ))
    record disconnect_ui_connect_button_after_tap_ms_px $(( ${tui:-0} - da ))
    sleep 1; save_log "$tb"
    ;;
S4)
    # Exit switch while connected: pick another city in the picker.
    ensure_connected
    current="$(ui_find '^Amsterdam$' >/dev/null && echo Amsterdam || echo other)"
    target="${TARGET_CITY:-Helsinki}"
    ui_tap '^(Switch location|Amsterdam|Helsinki|Falkenstein)$' >/dev/null
    ui_wait '^Select location$' 10 >/dev/null
    sleep 1
    gfx_reset
    read -r tb ta <<<"$(ui_tap "^$target$")"
    tdisp="$(wait_log "$tb" 'dispatched reconnect intent' 20)"
    tup="$(wait_log "$tb" 'multi-hop tunnel up' 60)"
    sleep 2; save_gfx
    tdown="$(log_first "$tb" 'multi-hop tunnel cancelled by Kotlin|multi-hop session ended')"
    tnat="$(log_first "$tb" 'multi-hop connect \(')"
    record target "$target"
    record dispatch_after_tap_ms $(( ${tdisp:-0} - ta ))
    record old_session_down_after_tap_ms $(( ${tdown:-0} - ta ))
    record native_connect_after_tap_ms $(( ${tnat:-0} - ta ))
    record tunnel_up_after_tap_ms $(( ${tup:-0} - ta ))
    save_log "$tb"
    ;;
S5)
    # Picker typing: "ne", "net", "neth", then clear.
    go_home_screen
    ui_tap '^(Switch location|Amsterdam|Helsinki|Falkenstein)$' >/dev/null
    ui_wait '^Select location$' 10 >/dev/null; sleep 1
    ui_tap 'Search locations or servers' >/dev/null; sleep 0.8
    gfx_reset
    t0="$(now_ms)"
    dsh input text ne; sleep 0.7
    dsh input text t; sleep 0.7
    dsh input text h; sleep 0.7
    for _ in 1 2 3 4; do dsh input keyevent KEYCODE_DEL; sleep 0.25; done
    sleep 0.8
    save_gfx
    dsh input keyevent KEYCODE_BACK; sleep 0.5; dsh input keyevent KEYCODE_BACK; sleep 0.8
    go_home_screen
    ;;
S6)
    # Navigation: home > Settings > VPN settings > back > back.
    go_home_screen; sleep 1
    gfx_reset
    ui_tap '^Settings$' >/dev/null; sleep 1
    ui_tap '^VPN settings$' >/dev/null; sleep 1
    dsh input keyevent KEYCODE_BACK; sleep 1
    dsh input keyevent KEYCODE_BACK; sleep 1
    save_gfx
    ;;
S7|S7b)
    # Idle 60 s: S7 connected, S7b disconnected (control).
    if [ "$SC" = S7 ]; then ensure_connected; else ensure_disconnected; fi
    sleep 5
    pid="$(app_pid)"
    snap() { dsh "cat /proc/$pid/stat | awk '{print \$14+\$15}'; for t in /proc/$pid/task/*; do grep -E 'nr_switches|nr_voluntary_switches' \$t/sched 2>/dev/null; done | awk '{s[\$1]+=\$3} END {for (k in s) print k, s[k]}'; ls /proc/$pid/task | wc -l" | tr -d '\r'; }
    gfx_reset
    t0="$(now_ms)"
    before="$(snap)"
    sleep 60
    after="$(snap)"
    t1="$(now_ms)"
    echo "$before" > "$OUT_DIR/$SC-$IT-before.txt"; echo "$after" > "$OUT_DIR/$SC-$IT-after.txt"
    cpu_b="$(echo "$before" | sed -n 1p)"; cpu_a="$(echo "$after" | sed -n 1p)"
    sw_b="$(echo "$before" | awk '$1=="nr_switches"{print $2}')"; sw_a="$(echo "$after" | awk '$1=="nr_switches"{print $2}')"
    vs_b="$(echo "$before" | awk '$1=="nr_voluntary_switches"{print $2}')"; vs_a="$(echo "$after" | awk '$1=="nr_voluntary_switches"{print $2}')"
    record window_ms $(( t1 - t0 ))
    record cpu_ticks_10ms $(( cpu_a - cpu_b ))
    record cpu_percent_of_one_core "$(LC_ALL=C awk -v d=$(( cpu_a - cpu_b )) -v w=$(( t1 - t0 )) 'BEGIN{printf "%.2f", d*10/w*100}')"
    record context_switches_per_s "$(LC_ALL=C awk -v d=$(( sw_a - sw_b )) -v w=$(( t1 - t0 )) 'BEGIN{printf "%.1f", d*1000/w}')"
    record voluntary_switches_per_s "$(LC_ALL=C awk -v d=$(( vs_a - vs_b )) -v w=$(( t1 - t0 )) 'BEGIN{printf "%.1f", d*1000/w}')"
    record threads "$(echo "$after" | tail -1)"
    record rust_log_lines_per_min "$(logcat_since "$t0" | grep -c ' WarrenJni')"
    save_gfx
    record pss_total_kb "$(dsh dumpsys meminfo "$PKG" | tr -d '\r' | awk '/TOTAL PSS:/{print $3}')"
    ;;
S8)
    # Forum sign-in by code with a fake session id: the status preflight answers 404.
    # S8_CONNECTED=1 runs it over a live tunnel (the forum transport bypasses it).
    if [ "${S8_CONNECTED:-0}" = 1 ]; then ensure_connected; else ensure_disconnected; fi
    ui_tap '^Settings$' >/dev/null; sleep 1
    ui_tap '^Sign in to the forum with a code$' >/dev/null; sleep 1.2
    ui_tap '^Sign-in code$' >/dev/null; sleep 0.6
    code="$(LC_ALL=C tr -dc 'a-f0-9' < /dev/urandom | head -c 32)"
    dsh input text "$code"; sleep 0.5
    read -r cb ca <<<"$(ui_tap '^Continue$')"
    tprompt="$(ui_wait '^Approve sign-in$' 15)"
    record prompt_after_continue_ms $(( ${tprompt:-0} - ca ))
    sleep 0.5
    read -r tb ta <<<"$(ui_tap '^Approve sign-in$')"
    tres="$(wait_log "$tb" 'WarrenForumLoginUseCase: sign-in not approved|forumLogin: provider answered|forumLogin: transport error' 40)"
    tgone="$(log_first "$tb" 'forumLogin: session already gone before signing|forumLogin: status preflight')"
    record preflight_verdict_after_approve_ms $(( ${tgone:-0} - ta ))
    record result_after_approve_ms $(( ${tres:-0} - ta ))
    sleep 2
    save_log "$cb"
    f="$(ui_dump)"; python3 -c "
import sys,xml.etree.ElementTree as ET
r=ET.parse(sys.argv[1]).getroot()
print(' / '.join((n.get('text') or '')[:60] for n in r.iter('node') if (n.get('text') or '') and n.get('bounds','')>'[0,700]')[:300])" "$f" > "$OUT_DIR/$SC-$IT-result-ui.txt"
    dsh input keyevent KEYCODE_BACK; sleep 0.6; go_home_screen
    ;;
S9)
    # Report collection: Settings > Report a problem > View the logs.
    go_home_screen
    ui_tap '^Settings$' >/dev/null; sleep 1
    ui_tap '^Report a problem$' >/dev/null; sleep 1.2
    read -r tb ta <<<"$(ui_tap '^View the logs$')"
    # The title band right of "Logs to be sent" is empty on the preview and
    # carries the tail of "Report a problem" before it (715 bright pixels).
    tprev="$(region_wait 520 205 600 245 lt 50 60)"
    tcoll="$(wait_log "$tb" 'collectProblemReport: [0-9]+ bytes in' 60)"
    line="$(logcat_since "$tb" | grep -E 'collectProblemReport: [0-9]+ bytes in' | head -1)"
    record report_bytes "$(echo "$line" | grep -oE '[0-9]+ bytes' | grep -oE '[0-9]+')"
    record rust_collect_ms "$(echo "$line" | grep -oE 'in [0-9]+ ms' | grep -oE '[0-9]+')"
    record collect_done_after_tap_ms $(( ${tcoll:-0} - ta ))
    record preview_visible_after_tap_ms_px $(( ${tprev:-0} - ta ))
    sleep 1; save_log "$tb"
    dsh input keyevent KEYCODE_BACK; sleep 0.8; dsh input keyevent KEYCODE_BACK; sleep 0.8; go_home_screen
    ;;
S10)
    # Network handover: drop wifi and data, restore, time to the tunnel being up again.
    ensure_connected
    t0="$(now_ms)"
    toff="$(dsh 'svc wifi disable; svc data disable; date +%s%3N' | tr -d '\r')"
    sleep 10
    ton="$(dsh 'svc wifi enable; svc data enable; date +%s%3N' | tr -d '\r')"
    tback="$(wait_log "$ton" 'underlying network changed|dialable network is back' 60)"
    tup="$(wait_log "$ton" 'multi-hop session re-established|multi-hop tunnel up' 90)"
    tip="$(wait_log "$ton" 'setup-stream returned IpAssign' 30)"
    sleep 3
    save_log "$t0"
    record network_change_seen_after_enable_ms $(( ${tback:-0} - ton ))
    record session_reestablished_after_enable_ms $(( ${tup:-0} - ton ))
    record ipassign_after_enable_ms $(( ${tip:-0} - ton ))
    record lost_marker_after_disable_ms $(( $(log_first "$toff" 'underlying network lost') - toff ))
    record forced_reconnect_after_disable_ms $(( $(log_first "$toff" 'forced supervisor reconnect') - toff ))
    record watchdog_recovered_ms_after_route_change "$(grep -oE 'recovered via forced re-handshake [0-9]+ ms' "$OUT_DIR/$SC-$IT-logcat.txt" | head -1 | grep -oE '[0-9]+ ms' | grep -oE '[0-9]+')"
    record supervisor_reconnect_duration_ms "$(grep -oE 're-established duration_ms=[0-9]+' "$OUT_DIR/$SC-$IT-logcat.txt" | head -1 | grep -oE '[0-9]+$')"
    dsh 'svc wifi enable; svc data enable' >/dev/null
    ;;
*) echo "unknown scenario $SC" >&2; exit 2 ;;
esac
