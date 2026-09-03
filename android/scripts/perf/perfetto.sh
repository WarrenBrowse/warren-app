#!/usr/bin/env bash
# perfetto.sh start <name> <duration_ms> | pull <name>
# Records a system trace on the device (ftrace sched, atrace gfx/view/input/am
# for the app, frame timeline, logcat tags) detached, so a scenario can run
# while it captures.
set -u
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cmd="$1"; name="$2"
case "$cmd" in
start)
    dur="${3:-30000}"
    cat > "$OUT_DIR/$name.pbtx" <<EOF
buffers { size_kb: 131072 fill_policy: RING_BUFFER }
data_sources { config { name: "linux.ftrace" ftrace_config {
  ftrace_events: "sched/sched_switch"
  ftrace_events: "sched/sched_waking"
  ftrace_events: "power/cpu_frequency"
  ftrace_events: "binder/binder_transaction"
  atrace_categories: "gfx" atrace_categories: "view" atrace_categories: "input"
  atrace_categories: "am" atrace_categories: "binder_driver" atrace_categories: "dalvik"
  atrace_apps: "$PKG" } } }
data_sources { config { name: "android.surfaceflinger.frametimeline" } }
data_sources { config { name: "android.log" android_log_config {
  filter_tags: "WarrenJni" filter_tags: "warren" filter_tags: "ActivityTaskManager" filter_tags: "Choreographer" } } }
data_sources { config { name: "linux.process_stats" process_stats_config { scan_all_processes_on_start: true } } }
duration_ms: $dur
EOF
    adbs push "$OUT_DIR/$name.pbtx" /data/misc/perfetto-configs/$name.pbtx >/dev/null
    dsh "perfetto --background --txt -c /data/misc/perfetto-configs/$name.pbtx -o /data/misc/perfetto-traces/$name.pftrace" 2>&1 | tail -1
    ;;
pull)
    adbs pull /data/misc/perfetto-traces/$name.pftrace "$OUT_DIR/$name.pftrace" 2>&1 | tail -1
    ;;
esac
