#!/usr/bin/env python3
"""Summarise a Perfetto trace of the Warren app: longest main-thread slices,
frame timeline jank, and per-second wakeups of the app's threads."""
import sys
from perfetto.trace_processor import TraceProcessor

path = sys.argv[1]
pkg = sys.argv[2] if len(sys.argv) > 2 else "com.warrenbrowse.vpn.beta"
tp = TraceProcessor(trace=path)

def q(sql):
    return list(tp.query(sql))

print("== process ==")
for r in q(f"select upid, pid, name from process where name like '{pkg}%'"):
    print(r.upid, r.pid, r.name)

print("== main-thread slices > 8 ms (top 25) ==")
rows = q(f"""
select s.ts, s.dur, s.name
from slice s join thread_track tt on s.track_id = tt.id
join thread t using(utid) join process p using(upid)
where p.name like '{pkg}%' and t.is_main_thread = 1 and s.dur > 8000000
order by s.dur desc limit 25""")
for r in rows:
    print(f"{r.dur/1e6:8.1f} ms  {r.name}")
if not rows:
    print("(none)")

print("== all slices > 30 ms in the app process, any thread (top 25) ==")
for r in q(f"""
select s.dur, s.name, t.name as tname
from slice s join thread_track tt on s.track_id = tt.id
join thread t using(utid) join process p using(upid)
where p.name like '{pkg}%' and s.dur > 30000000
order by s.dur desc limit 25"""):
    print(f"{r.dur/1e6:8.1f} ms  [{r.tname}] {r.name}")

print("== frame timeline (actual frames of the app) ==")
for r in q(f"""
select count(*) as n, sum(case when jank_type != 'None' then 1 else 0 end) as janky,
       max(dur)/1e6 as max_ms
from actual_frame_timeline_slice a join process p using(upid)
where p.name like '{pkg}%'"""):
    print(f"frames={r.n} janky={r.janky} max_dur_ms={r.max_ms}")
for r in q(f"""
select jank_type, count(*) as n
from actual_frame_timeline_slice a join process p using(upid)
where p.name like '{pkg}%' group by jank_type order by n desc"""):
    print(f"  {r.jank_type}: {r.n}")

print("== app thread wakeups per second (sched_waking targets) ==")
for r in q(f"""
select t.name as tname, count(*) as n,
       (max(w.ts) - min(w.ts)) / 1e9 as span_s
from thread_state w join thread t using(utid) join process p using(upid)
where p.name like '{pkg}%' and w.state = 'R'
group by t.name order by n desc limit 15"""):
    print(f"  {r.tname:30s} runnable-transitions={r.n} over {r.span_s:.1f}s")

print("== trace bounds ==")
for r in q("select start_ts, end_ts, (end_ts-start_ts)/1e9 as s from trace_bounds"):
    print(f"duration={r.s:.1f}s")
