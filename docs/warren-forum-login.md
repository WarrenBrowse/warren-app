# Community forum from the app: sign-in, in-app report, logs

The forum (`forum.warrenbrowse.com`) authenticates with the Warren wallet
through the `warren-connect` broker (design: warren-core doc 55). This file is
the app-side reference: which piece lives where, what can fail, and how a user
who cannot get through the browser still reaches the support team.

## The three flows

| flow | trigger | what the app signs | server route |
|---|---|---|---|
| Forum sign-in | `<scheme>://forum-login?sid&host(&xd=1)` deep link, or a sign-in code typed under Settings | `{"sid":...}` | `POST /v1/forum/login` |
| Attach logs to a topic | `<scheme>://attach-logs?sid&topic&host` (desktop today) | `{"sid","topic_id","log_gz_b64"}` | `POST /v1/forum/attach-logs` |
| In-app report | Settings, "Report a problem" (Android today) | the form fields plus `log_gz_b64` | `POST /v1/forum/report` |

The scheme is per product environment (`warren`, `warren-beta`,
`warren-staging`); connect emits the one its `WARREN_DEEP_LINK_SCHEME` names.
The host is a hard allowlist (`connect.warrenbrowse.com`) on every platform.

## Where the logic lives

- `warren-forum` (Rust, shared by `warren-jni` and `warren-ios`): the
  allowlist, the sid shape, the sign-in code normalisation, the signed request
  bytes for login and report, the status URL used as a clock preflight, the
  clock offset read from a `Date` header, the outcome tables and the FFI
  envelopes. Host-tested; iOS and Android cannot drift on the wire.
- `warren-jni/src/forum_android.rs`: the Android exports. The forum POSTs
  ride the VpnService-protected transport (`protected_transport.rs`) when the
  crate is built with the tunnel feature: a plain socket is routed into the
  TUN, and a TUN that is still coming up, blocking or wedged swallows it (the
  token mint met that first). With no VPN service alive the protector is a
  pass-through.
- `warren-jni/src/rust_log.rs`: the Rust log file (`files/rust_logs/warren.log`,
  daemon line format, one rotation per process, 4 MiB cap) fed next to logcat
  by every `log` and `tracing` record of the engine and the bridge.
- `warren-jni/src/report.rs` + `mullvad-problem-report`: the redacted report,
  the same collector and redaction as the desktop, with the Android metadata
  header from Kotlin.
- Android Kotlin (`app/forum/`): the deep-link classification (rejections are
  logged by class, never by value), the consent prompt, the events journal
  (`android_app_logs/warren-events.log`, one JSON line per step; its fields
  are the typed `JournalField`s, so a line has no place for a sid or a
  handle), the platform diagnostics (`ForumDiagnostics`, a fact table over
  the `ForumPlatformReads` seam whose device reads live in
  `AndroidForumPlatformReads` and whose JVM tests use a fake), the report
  submitter; the screens live in `lib/feature/settings/impl/support/`.
- Desktop: `desktop/packages/mullvad-vpn/src/main/forum-*.ts` plus the daemon
  `SignForum*` RPCs; see the files' headers.

## The sign-in, step by step (Android)

1. The browser shows the approval page and the user taps the button. Chromium
   fires the intent; Firefox raises its own "Open this link in Warren VPN Beta
   app?" sheet first, and a cancelled sheet (or "Open links in apps: Never")
   leaves the page polling forever with nothing reaching connect. That was the
   whole population of failed Android sign-ins between 2026-08-19 and 09-02.
   The page now notices a tap that did not leave the page and shows the
   session id as a code; the app accepts it under Settings, "Sign in to the
   forum with a code", which raises the same consent prompt.
2. `MainActivity` classifies the link and stashes it; the launching intent is
   consumed once (a recreated activity must not re-prompt for a consumed sid).
3. The consent prompt is explicit. Approve first reads the tunnel state
   (`ForumPreflight` in `lib/repository`, shared with the report): the forum
   POST bypasses the TUN, but the connect host name goes through the system
   resolver, which a tunnel coming up leaves timing out and a kill switch
   holding leaves with no server at all. Connecting, reconnecting,
   disconnecting, failed and blocking defer the attempt with its own message
   and keep the prompt open for a retry (`login.deferred` in the journal, the
   state class only); connected and disconnected proceed: the mnemonic is
   read silently and handed to `WarrenJni.forumLogin`.
4. Rust reads `GET /v1/session/{sid}/status` once: a 404 is reported as
   `expired` without spending a signature; the `Date` header corrects a device
   clock outside the server's 60 s window (the 2026-08-18 class) before the
   signed POST.
5. The approved body carries the forum identity (`handle`, `notify_slot`);
   Android stores it (`ForumIdentityRepository`) and shows the "Forum name"
   on the account page. The app then moves to the background so the browser
   page, which only re-polls when visible, completes the login.

Every step writes its class to the events journal and the Rust log, so a
report filed afterwards explains a failure without a second round trip.

## The in-app report (Android)

Settings, "Report a problem" mirrors the forum's "Report a bug" form (area,
what happened, steps, frequency). "Include technical logs" is on by default and
"View the logs" shows the exact file about to be sent. Send runs the same
tunnel-state preflight as the sign-in before collecting anything
(`report.deferred`, the form left intact), then collects a fresh report, gzips
it (12 MiB cap, the desktop's), signs and POSTs it. The broker
creates the wallet's forum account when needed, opens the topic under the
handle, tags the platform, and delivers the logs to the staff like the
paperclip flow. Outcomes: created (with the topic link), never paid (routes to
the website help form), clock skew, rate limited, too large (offers a resend
without logs), invalid, server error, generic.

What the report header carries, all safe by construction: app build and
scheme, installer, ROM and GMS/microG verdict, auto-time and clock, the
packages that handle the deep link and the one that resolves it, the default
browser, battery and background restrictions, tunnel and VPN service state,
network transports and validation, private DNS mode, wallet state, and the
class of the last sign-in result read back from the events journal. Never an
address, a sid, a handle or an SSID.

## Reading a failed attempt

- Device: `adb logcat -s WarrenJni:V` for the live run;
  `files/rust_logs/warren.log`, `files/android_app_logs/warren-events.log`
  for the history; both are inside every in-app report.
- Forum host: `docker logs warren-warren-connect-1 | grep -E "forum login|in-app report"`,
  and the connect edge log for a POST that never arrived.
