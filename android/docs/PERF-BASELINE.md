# Android performance baseline (Pixel_10 AVD, 2026-09-03)

The first measured run of the profiling plan in the Android performance
review (scenarios S1 to S7) plus the three scenarios the critique added
(S8 forum sign-in, S9 report collection, S10 network handover). Every
number here was measured on the emulator named below with the scripts in
`android/scripts/perf/`; nothing is estimated. The thresholds column repeats
what the review proposed and is **proposed, not yet a gate**: no CI job fails
on any of them, and the first fix lots (H1, H2/H5, H3, H4) are measured
against this table, before and after.

The **P2** columns are the second run, after the first fix lot (H1 and
H2/H5: `cc5fc6a6e8` to `b2fb915395`); the section "P2 re-measurement" at the
end says what changed, how it was measured and what the numbers do not say.

## Machine

| item | value |
|---|---|
| host | Apple M4, 24 GB, macOS (Darwin 25.6.0), Android Emulator 37.1.11.0 |
| AVD | `Pixel_10`, system image `android-37.0` `google_apis_playstore_ps16k` arm64-v8a (Android 17, API 37, build `CE2A.260420.019`, Play image, not rootable) |
| CPU / RAM | 4 vCPU; `config.ini` says `hw.ramSize=2048` but the running instance reports `MemTotal` 4,062,880 kB, so 4 GB was in effect |
| display | 1080x2424 at 420 dpi, 60 Hz |
| graphics | host GPU (`hw.gpu.mode=auto`, `ro.hardware.egl=emulation`), HWUI pipeline `Skia (OpenGL)` |
| network | the Mac's uplink (Free, France) to `api.beta.warrenbrowse.com`, `connect.warrenbrowse.com` and the Amsterdam / Helsinki beta exits; multi-hop on, 20 Mbps beta cap |

## Build

| item | value |
|---|---|
| commit | `b317eaf496` (`feat(android): report the Connect screen fully drawn`), version `1.1.4-dev-b317ea` |
| variant | `betaBenchmarkRelease`: `cd android && ./gradlew :app:assembleBetaBenchmarkRelease -Pwarren.app.build.cargo.targets=arm64` (the variant exists since `43eb0712fd`, which extended `baselineFilter` to the beta flavor) |
| shape | R8 full mode, resource shrinking, `profileable android:shell`, not debuggable (`dumpsys package` flags `HAS_CODE ALLOW_CLEAR_USER_DATA`), Rust `release` profile (`opt-level = "s"`, LTO, data-plane crates at `opt-level = 3`) |
| sizes | arm64 `libwarren_jni.so` 10,475,688 bytes (18.5 MB in a debug build); `classes.dex` 5,500,852 bytes. The 39.6 MB APK is not representative: an arm64-only cargo build leaves the other three ABIs' debug `.so` files in `rustJniLibs`, and the packager takes them |
| signing | the dev keystore path works (`apksigner` on the first build: `CN=Warren VPN Dev`); the APK actually installed for the run was re-signed with the debug key so `adb install -r -t -d` updated the logged-in beta app in place instead of wiping its wallet (same applicationId, different certificate refuses the update) |
| ART state | `dumpsys package dexopt`: `arm64: [status=verify] [reason=install]`, so no ahead-of-time code; the checked-in baseline profile names Mullvad classes only and matches nothing |
| session | a paid test wallet logged in, forum identity learnt, exit pinned to Amsterdam, multi-hop on, DAITA off, port forwarding on |

Measured 2026-09-03 between 00:15 and 01:05 UTC.

## Method

- Time is read on the device clock (`date +%s%3N`) and correlated with
  `logcat -v epoch`; there is no host/guest skew in any delta.
- A tap is `input tap` from `adb shell`; the recorded tap time is the device
  time when that command returned (7 to 12 ms after the time before it), so
  the tap latencies below exclude the command's own start.
- Engine markers come from the Rust side (`WarrenJni` tag, `Debug` level in
  this build): `multi-hop connect (`, `multi-hop tunnel up`, `multi-hop tunnel
  cancelled by Kotlin`, `multi-hop session re-established`, `setup-stream
  returned IpAssign`, `collectProblemReport: N bytes in M ms`, and from the
  Kotlin side (`warren` tag, `Info`): `dispatched Quinn connect intent`,
  `dispatched reconnect intent`, `underlying network changed`,
  `WarrenForumLoginUseCase: sign-in not approved`.
- Frame statistics are `dumpsys gfxinfo <pkg> reset` before the scenario and
  the `Total frames rendered` / `Janky frames` / percentile block after it.
  The GPU percentiles of `gfxinfo` sit in the 4950 ms overflow bucket on this
  AVD for every scenario and are not reported.
- Screen transitions that no log announces are timed by polling. A
  `uiautomator dump` costs about 1.9 s here, so the values marked
  `ui-poll` are upper bounds with that granularity; the values marked
  `px-poll` watch one pixel or a region through a raw `screencap`, 0.3 s per
  sample.
- At least three iterations per scenario (four for S4 so the pin ends where
  it started, eleven for S2 across two series); median and worst are
  reported, nothing discarded. The first S1 iteration ran right after the
  install and is the worst one.

Reproduce: `bash android/scripts/perf/run-baseline.sh` (all scenarios) or
`bash android/scripts/perf/scenarios.sh S2 1` (one iteration), with
`emulator-5554` running the release-shaped beta app. A Perfetto trace of one
run: `bash android/scripts/perf/perfetto.sh start s2 40000`, run the scenario,
`bash android/scripts/perf/perfetto.sh pull s2`, then
`python3 android/scripts/perf/trace_report.py <trace>` (needs `pip install
perfetto`).

## Results

### S1 cold start (`am force-stop`, then `am start -W`; page cache not dropped, no root)

| metric | median | worst | runs | P2 median | P2 worst | P2 runs | proposed threshold (AVD) |
|---|---|---|---|---|---|---|---|
| `TotalTime` (TTID) | 830 ms | 1247 ms | 1247, 830, 750 | 589 ms | 908 ms | 516, 589, 908 | <= 900 ms |
| `Fully drawn` (TTFD, new `ReportDrawn` on the Connect screen) | 830 ms | 1247 ms | same as TTID on every run | 589 ms | 908 ms | same as TTID | none proposed; equal to TTID today because the splash keeps the window until the Connect screen's first frame, so that frame is both |
| main-thread slice > 50 ms other than `bindApplication` | not measured (no trace of a cold start yet) | | | none |
| StrictMode disk violations on main | not measurable (release build) | | | zero |

### S2 connect, then disconnect (from the home screen, Amsterdam pinned)

| metric | median | worst | runs | P2 median | P2 worst | P2 runs | proposed threshold |
|---|---|---|---|---|---|---|---|
| tap to `dispatched Quinn connect intent` (config rebuild: `/v1/exits` + directory fetch, Keystore read; P2: no fetch, the catalogue and the directory are served from the hourly cache) | 226 ms | 612 ms | 277, 171, 188, 226, 206, 409, 185, 217, 244, 612, 365 | 10 ms | 272 ms | 272, 6, 86, 5, 10 | none (part of the 2.0 s budget) |
| tap to native `multi-hop connect (` (service start, config parse, `establish`, `connectTunnel`) | 482 ms | 874 ms | 482, 256, 405, 490, 463, 672, 447, 535, 453, 874, 672 | 467 ms | 1032 ms | 1032, 80, 467, 61, 499 | none |
| tap to `multi-hop tunnel up` (engine has an inner IP) | 867 ms | 1411 ms | 967, 867, 877, 854, 817, 1326, 853, 819, 809, 1411, 1329 | 1084 ms | 1504 ms | 1504, 461, 1084, 704, 1121 | <= 2.0 s tap to Connected (UI lands <= 250 ms later, poll granularity, H3) |
| dispatch to tunnel up | 689 ms | 964 ms | 690, 696, 689, 628, 611, 917, 668, 602, 565, 799, 964 | 998 ms | 1232 ms | 1232, 455, 998, 699, 1111 | none |
| disconnect tap to `multi-hop tunnel cancelled by Kotlin` | 21 ms | 35 ms | 14, 16, 27, 35, 20, 16, 31, 21, 27, 1, 24 | 36 ms | 75 ms | 28, 36, 75, 28, 55 | none |
| disconnect tap to the Connect button back on screen (`px-poll`, last six runs) | 1336 ms | 1623 ms | 1326, 1623, 1291, 1316, 1345, 1506 | 2367 ms | 5130 ms | 5130, 2508, 2367, 1946, 2363 | none (the card stays on `Disconnecting` until the adapter has seen the session close) |
| tap to first `Connecting` frame | not measured (needs a frame-level marker) | | | | | | <= 1 frame |
| main-thread slice > 16 ms in the 2 s after the tap | see the Perfetto section: the longest non-drawing main-thread slices are `serviceUnbind` 23 ms, `serviceStart` (connect intent) 14 ms, `binder transaction` 12 ms | | | P2 trace: `Recomposer:recompose` 55 ms, bitmap decode 33 ms (H4), `serviceUnbind` 30 ms, `binder transaction` 28 ms, `serviceStart` 11 ms; the adapter's own Binder work (`establish`) now shows on `DefaultDispatch` (9 transactions, longest 27 ms) and no longer on main | | | none > 16 ms |

Eleven runs: the first five in one series (one under Perfetto), six more
later in the session once the disconnect wait had moved to pixel polling;
the later series ran while the beta network was slower (three of its six
tunnel-up times above 1.3 s against none in the first five).

### S3 connecting animation (frames of the 8 s after the connect tap, measured on the S2 runs)

| metric | median | worst | runs | proposed threshold |
|---|---|---|---|---|
| frames rendered in the window (11 runs) | 106 | 113 | 93, 106, 113, 98, 111, 111, 95, 113, 103, 107, 101 | about 480 at 60 Hz |
| janky frames | 99.0 % | 100 % | 100, 100, 95.6, 100, 96.4, 95.5, 100, 95.6, 99.0, 100, 98.0 % | <= 5 % |
| frame time P50 | 150 ms | 150 ms | 150, 125, 125, 150, 125, 129, 150, 125, 150, 150, 150 | |
| frame time P90 | 300 ms | 400 ms | 400, 350, 250, 300, 250, 250, 300, 300, 300, 350, 300 | |
| frame time P95 | 450 ms | 600 ms | 600, 500, 450, 600, 450, 450, 500, 450, 400, 450, 500 | P95 <= 12 ms RenderThread |
| frame time P99 | 700 ms | 850 ms | 700, 700, 650, 850, 800, 750, 750, 800, 600, 550, 550 | P99 <= 16 ms |
| recompositions of `SceneryBackdrop` | not measured (Layout Inspector only) | | | zero |

The blur-and-zoom phase runs at 7 to 8 frames per second on this AVD. The
Perfetto trace shows where the time goes: over a 40 s S2 window the main
thread ran for 0.35 s in total, the RenderThread for 7.35 s, and the longest
slices are `DrawFrames` 443 ms and `waiting for GPU completion` 419 ms.
The frame timeline classifies all 130 app frames of that window as janky,
the dominant classes being `App Deadline Missed` combined with SurfaceFlinger
scheduling. So on the emulator the connecting animation is GPU-bound in the
emulated GL pipeline (H4), and the UI thread is idle; a phone's GPU will
change the absolute numbers, the structure (three full-screen blur passes
per frame for 6 s) is the same.

### S4 exit switch while connected (picker, tap another city; the picker pops on the pick)

| metric | median | worst | runs | P2 median | P2 worst | P2 runs | proposed threshold |
|---|---|---|---|---|---|---|---|
| pick tap to `dispatched reconnect intent` | 0 ms (inside the tap command) | | -2, -3, -2, -2 | 0 ms | | -5, -7, -5, -3 | |
| pick tap to old session torn down (`cancelled by Kotlin`; the config rebuild's two fetches come first; P2: no fetch) | 452 ms | 25,172 ms | 25172, 411, 410, 492 | 25 ms | 169 ms | 15, 35, 169, 14 | |
| pick tap to native `multi-hop connect (` (400 ms settle + `awaitTunnelClosed`) | 1226 ms | 25,810 ms | 25810, 991, 1352, 1100 | 505 ms | 692 ms | 510, 495, 692, 499 | |
| pick tap to `multi-hop tunnel up` | 1798 ms | 26,339 ms | 26339, 1656, 1940, 1511 | 1262 ms | 1551 ms | 1551, 1001, 1522, 802 | <= 2.5 s (<= 1.5 s after the cache and settle fixes) |
| HTTPS requests issued inside the switch window (h2 request headers sent, from the Rust debug log) | 3 (picker refresh, `/v1/exits`, directory) | | | 0 to 1 | | the remaining one is the `/v1/subscription` refresh the Connect screen issues on re-entry, not the switch | 0 |
| janky frames during the switch (picker pop + card + scenery crossfade) | 95.8 % | 96.3 % | 95.6, 94.6, 96.3, 96.0 % | 100 % | 100 % | see the host-load caveat | |
| frame time P50 / P99 | 150 / 575 ms | 150 / 600 ms | | 300 / 875 ms | 350 / 1300 ms | see the host-load caveat | no main-thread slice > 16 ms |
| a `Disconnected` frame visible between the pick and the new `Connected` | not observed in the logs (the reconnect bridge held); not verified frame by frame | | | same | | | none visible |

Run 1 is the outlier the review predicted at H5 (a): the `/v1/exits` fetch
through the tunnel received an h2 `GOAWAY` right after the handshake, three
`listRelays` retries failed, the directory fetch timed out after 25 s, the
service logged `config rebuild unavailable ... reusing the cached config`
and re-dialled the **old** exit while the picker already showed the new
one. The user saw a 26 s switch that landed on the exit they left, with no
error. Runs 2 to 4 are the normal path: 1.5 to 1.9 s, of which 0.4 s is the
network round trips and about 0.6 s the fixed settle plus close wait.

The P2 lot reproduced run 1 on its first switch too, and the Rust debug log
named the cause: `hyper_util::client::legacy::pool: reuse idle connection`.
The shared reqwest client kept the h2 connection it had opened to the API
**before** the TUN came up; once the VpnService routes were installed that
TCP flow left through the exit under another source address, the server
never answered it, and each of the two requests that reused it died in the
transport's 15 s timeout (the `GOAWAY` is hyper closing the dead connection
afterwards). It is not a network event: it happens on the first API request
after every TUN transition, deterministically. `64b0c40522` retires the
pool at `connectTunnel`, `disconnectTunnel` and `notifyNetworkChanged`, and
`058250de16` removes the two requests from the switch altogether, so the
four P2 switches show neither the stall nor the fetches.

### S5 picker typing ("ne", "net", "neth", four deletes; 6 relays in the catalogue)

| metric | median | worst | runs | proposed threshold |
|---|---|---|---|---|
| frames rendered | 48 | 48 | 48, 48, 47 | |
| janky frames | 75 % | 76.6 % | 70.8, 75.0, 76.6 % | frame <= 16 ms per keystroke |
| frame time P50 / P90 / P99 | 42 / 61 / 69 ms | 42 / 69 / 81 ms | | |
| recompositions per keystroke | not measured | | | <= 1 |

With six relays the list recomputation is negligible; the 40 ms frames
are the keyboard and the list animation on the emulated GPU.

### S6 navigation (home > Settings > VPN settings > back > back)

| metric | median | worst | runs | proposed threshold |
|---|---|---|---|---|
| frames rendered (four 250 ms transitions) | 49 | 56 | 49, 49, 56 | about 60 at 60 Hz |
| janky frames | 53.1 % | 67.4 % | 53.1, 67.4, 48.2 % | 0 |
| frame time P50 / P90 / P99 | 42 / 150 / 200 ms | 48 / 150 / 200 ms | | |

### S7 idle, 60 s, connected (and S7b the same window disconnected, one run, as the control)

| metric | S7 median | S7 worst | S7 runs | S7b (disconnected) | proposed threshold |
|---|---|---|---|---|---|
| process CPU, share of one core | 0.13 % | 0.13 % | 0.13, 0.12, 0.13 | 0.02 % | |
| context switches per second, all threads | 25.6 /s | 26.3 /s | 26.3, 22.7, 25.6 | 3.5 /s | <= 2 wakeups/s after H3; the review expected >= 4 today |
| voluntary switches per second | 24.8 /s | 25.4 /s | 25.4, 22.3, 24.8 | 3.0 /s | |
| `WarrenJni` logcat lines per minute | 59 | 88 | 59, 56, 88 | 3 | |
| frames rendered | 0 | 0 | | 0 | |
| threads | 57 | | | 58 | |
| PSS after the window | 91.4 MB | 92.2 MB | 92.2, 91.4, 91.4 | 108.6 MB | <= 220 MB on the Connect screen after 10 switches (7 reconnects preceded this) |

The connected process wakes about 25 times a second while nothing is
displayed: the 250 ms status poll (four JNI calls per tick, H3), the QUIC
timers, the egress probe and the `quinn::connection: drive` debug lines the
release build still writes to logcat (about one per second). The emulator
cannot say what this costs in battery; see the last section.

### S8 forum sign-in by code (fake 32-hex code; the status preflight answers 404; tunnel disconnected)

| metric | median | worst | runs | proposed threshold |
|---|---|---|---|---|
| Continue tap to the consent prompt on screen (`ui-poll`, upper bound) | <= 2536 ms | <= 3270 ms | 2515, 2536, 3270 | none proposed |
| Approve tap to the preflight verdict (`session already gone`: one TLS handshake to `connect.warrenbrowse.com` on the pool-less transport, plus the GET) | 1364 ms | 1442 ms | 1442, 1364, 1122 | none proposed |
| Approve tap to the outcome on the Kotlin side (`sign-in not approved`) | 1365 ms | 1442 ms | 1442, 1365, 1123 | none proposed |
| the same, with the tunnel connected (the forum transport is socket-protected, so it leaves outside the tunnel) | 1318 ms | 1655 ms | 1655, 1318, 402 | none proposed |

The server answers the 404 within the same round trip; the 1.1 to 1.7 s is
the connection setup (root store build, TCP, TLS 1.3) plus one request. A
real sign-in pays this twice (preflight, then the signed POST). The prompt
showed the expected "This sign-in request has expired" line on every run.

### S9 report collection (Settings > Report a problem > View the logs; logs included by default)

| metric | median | worst | runs | proposed threshold |
|---|---|---|---|---|
| redacted report size (plain text, before gzip) | 1,578,452 bytes | 1,585,692 bytes | 1571212, 1578452, 1585692 (a second series later in the session, after the logs had grown: 2004346, 2012150, 2022286) | none |
| Rust `collectProblemReport` duration (metadata, redaction, file assembly) | 49 ms | 54 ms | 49, 49, 54 | none |
| tap to `collectProblemReport` logged | 54 ms | 54 ms | 54, 52, 54 (second series, 2.0 MB: 73, 83, 73) | none |
| tap to the preview on screen (`px-poll`, second series, 2.0 MB report) | 1249 ms | 1397 ms | 1249, 1397, 1104 | none proposed |
| gzip size and duration | not measured: the gzip runs only on send, and no report was sent | | | |

The collection is cheap; the preview is what the user waits for, and the
wait is the preview screen reading the 2 MB file and laying it out as one
`Text` (`ReportPreviewScreen.kt`, `File(path).readText()` inside a
`LaunchedEffect`, on the main dispatcher): about 1.2 s between the Rust
result and the first preview frame.

### S10 network handover (connected; `svc wifi disable && svc data disable`, 10 s, then both enabled)

| metric | median | worst | runs | proposed threshold |
|---|---|---|---|---|
| disable to `underlying network lost` (Kotlin) | 264 ms | 283 ms | 264, 283, 233 | none proposed |
| disable to the watchdog's `forced supervisor reconnect` (3 s path validation) | 3556 ms | 3572 ms | 3556, 3572, 3534 | |
| enable to `underlying network changed` (Kotlin) | 100 ms | 107 ms | 107, 94, 100 | |
| enable to `multi-hop session re-established` | 1867 ms | 1886 ms | 1855, 1886, 1867 | none proposed |
| enable to `setup-stream returned IpAssign` (traffic flows) | 2025 ms | 2039 ms | 2005, 2039, 2025 | |
| watchdog's own `recovered ... after the default-route change` | 11,632 ms | 11,650 ms | measured from the loss, includes the 10 s outage | |
| supervisor's `re-established duration_ms` | 8343 ms | 8364 ms | from the forced reconnect, includes about 6.5 s of outage | |

The recovery is one supervisor redial once the network is back; nothing
waits on the Kotlin side. About 1.9 s from the network's return to a live
session is the exit handshake over the Mac's path (the emulator's Wi-Fi
comes back within the first 100 ms).

## Perfetto trace of one S2 run

The trace file is not committed (12 MB); the config is in `perfetto.sh` and
a new one takes a minute to record. Summary of the 40 s window (connect,
10 s connected, disconnect):

- main thread: 0.35 s running, 37.15 s sleeping; longest slices are all
  `Choreographer#doFrame` (up to 438 ms) whose time is `postAndWait`, the
  UI thread waiting for the RenderThread;
- RenderThread: 7.35 s running; `DrawFrames` up to 443 ms, `waiting for GPU
  completion` up to 419 ms;
- longest non-drawing main-thread slices: `serviceUnbind` 23.2 ms,
  `Record View#draw()` 16.3 ms, `serviceStart` (the connect intent) 13.9 ms,
  `binder transaction` 12.3 ms, `Recomposer:recompose` 12.1 ms;
- frame timeline: 130 frames, 130 janky (`App Deadline Missed` on 108 of
  them, most combined with `SurfaceFlinger Scheduling` or `Stuffing`);
- thread wake counts over the window: main 711, RenderThread 546,
  `DefaultDispatch` 469, `GPU completion` 244.

The `linux.perf` callstack sampling of the review's config was left out:
`perf_event_open` is not available to the shell on this image.

## P2 re-measurement (H1 and H2/H5, 2026-09-03)

Build `b2fb915395`, the same `betaBenchmarkRelease` variant, arm64 cargo
target and debug signing as the baseline, installed over the logged-in app
(`versionName 1.1.4-dev-b2fb91`). Commits under measurement:

- `cc5fc6a6e8` every tunnel transition (`establish`, `connectTunnel`, the
  teardown) runs on the adapter's own dispatcher; `onRevoke` waits on it with
  a 3 s bound, `onDestroy` does not wait;
- `30a5e6ec59` StrictMode watches the VPN service (the exemption is gone,
  with the dead split-tunnelling migration and the redundant library load
  that were its last two violations);
- `058250de16` the relay catalogue and the multi-hop directory follow the
  daemon's cadence (refetched once an hour, served from the snapshot in
  between: `RelayCatalog`, `warren-jni/src/directory_cache.rs`);
- `64b0c40522` one reqwest stack for every API call, retired at each TUN
  transition and handover (`warren-jni/src/api_transport.rs`);
- `b0d6e5f3e6` one manifest fetch for both the support gate and the upgrade
  prompt; `b2fb915395` the forum TLS configurations built once.

Method as above, S1 x3, S2 x5, S4 x4 (Helsinki, Amsterdam, Helsinki,
Amsterdam), one Perfetto trace of an S2 run, plus the request count of the
S4 window read from the Rust debug log (`send frame=Headers` lines against
`api.beta.warrenbrowse.com`). Measured 01:38 to 01:50 UTC.

**Host-load caveat.** The Mac ran other agents' Dart and Git tooling during
these runs: load average 4 at the start of the series, 16 at its end (the
baseline ran on an idle host). The S6 navigation control, which no commit in
this lot touches, rendered 37 frames at P50 93 ms against 49 frames at
P50 42 ms in the baseline, so every frame-time and every UI-polled figure
here (janky percentages, percentiles, the disconnect `px-poll`) is about half
as good as the same build would measure on an idle host and must not be read
as a regression or an improvement. The Perfetto trace agrees: the main thread
spent 0.35 s runnable-but-not-scheduled over the 34 s window against 0.58 s
running. The engine-marker timings (device clock, log to log) are affected
far less, and the two that this lot changed moved by an order of magnitude:

- tap to dispatch (S2) 226 ms to 10 ms: the config build makes no request;
- pick to old session down (S4) 452 ms to 25 ms, pick to tunnel up 1798 ms to
  1262 ms: the switch makes no request and no longer hits the dead pooled
  connection (no 26 s run in eight P2 switches, two lots);
- requests inside the switch window 3 to 0 (one run shows 1, the Connect
  screen's `/v1/subscription` refresh on re-entry, which is `ConnectScreen.kt`
  and not this lot);
- cold start TTID 830 ms to 589 ms median, with the load caveat above (the
  lot touched nothing on the startup path; the redundant `loadLibrary` and
  the service's migration only ran when the service started).

What the trace says about H1: the adapter's Binder work now appears on
`DefaultDispatch` (9 `binder transaction` slices, the longest 27 ms, which is
`establish()` creating the interface in `system_server`), and the
`serviceStart` slice of the connect intent on main is 11 ms of framework
bookkeeping. The main-thread slices still above 16 ms are the framework's
`serviceUnbind` (30 ms) and its `startForeground` transactions (28 ms),
Compose recomposition (55 ms) and the first scenery bitmap decode (33 ms,
H4): none is the tunnel transition, and the last two are L11d's.

StrictMode, on the debug build of the same commits, lists no violation whose
stack passes through `app.service` across a connect, a disconnect, two
switches and a network cut; the remaining violations are the
`SharedPreferences` reads of `WarrenLocalSettingsRepository`,
`AndroidKeystoreWalletRepository` and `SharedPreferencesForumIdentityRepository`
at construction on the main thread (H7, L11d).

## What this AVD cannot measure

- **Battery and wakeups as a phone sees them (H3).** No Battery Historian, no
  wakelock accounting worth reading; the 25 context switches per second above
  are the closest proxy. Only a physical device settles the H3 claim.
- **GPU time.** `gfxinfo` GPU percentiles overflow (4950 ms bucket) and the
  emulated GL pipeline makes every blur frame 100 ms or more, so S3, S4, S5
  and S6 frame numbers are comparable only with each other and with a later
  run on the same AVD, never with a phone. Thread affinity and the
  main-thread slices are trustworthy.
- **Radio latency.** The API and the exits are reached over the Mac's wired
  path (about 50 ms RTT to the exit); a 150 to 300 ms mobile path multiplies
  every network leg in S2, S4, S8 and S10.
- **A truly cold start.** The Play image has no root, so the page cache is
  never dropped; S1 is a warm-cache cold process start.
- **StrictMode.** Release builds carry none; the disk-on-main checks of the
  review need a debug build.
- **Refresh rate.** 60 Hz only; the 120 Hz budget of the review is untested.
- **Compose recomposition counts** (S3, S5): Layout Inspector in Android
  Studio, not scriptable here.
- **The h2 `GOAWAY` stall of S4 run 1** is a network event, reproducible only
  by chance; it is recorded, not characterised.
