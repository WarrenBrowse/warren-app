# Community forum from the app: sign-in, in-app report, logs

The forum (`forum.warrenbrowse.com`) authenticates with the Warren wallet
through the `warren-connect` broker (design: warren-core doc 55). This file is
the app-side reference: which piece lives where, what can fail, and how a user
who cannot get through the browser still reaches the support team.

## The three flows

| flow | trigger | what the app signs | server route |
|---|---|---|---|
| Forum sign-in | `<scheme>://forum-login?sid&host(&xd=1)` deep link, or a sign-in code typed under Settings | `{"login_version":2,"sid":...}` | `POST /v1/forum/login` |
| Attach logs to a topic | `<scheme>://attach-logs?sid&topic&host` (desktop today) | `{"sid","topic_id","log_gz_b64"}` | `POST /v1/forum/attach-logs` |
| In-app report | Settings, "Report a problem" (Android and desktop) | the form fields plus `log_gz_b64` | `POST /v1/forum/report` |

The scheme is per product environment (`warren`, `warren-beta`,
`warren-staging`); connect emits the one its `WARREN_DEEP_LINK_SCHEME` names,
and every client registers the one its `WARREN_PRODUCT_ENV` build compiles
in (the desktop table, the Android flavors and the iOS xcconfig are held to
the Rust `warren-product-env` crate by its `platform_lockstep` tests). The
host is a hard allowlist (`connect.warrenbrowse.com`) on every platform.

## Where the logic lives

- `warren-forum` (Rust, shared by `warren-jni` and `warren-ios`): the
  allowlist, the sid shape, the sign-in code normalisation, the signed request
  bytes for login and report, the status URL used as a clock preflight and
  the classing of its answer (`classify_status_preflight`), the clock offset
  read from a `Date` header, the outcome tables and the FFI envelopes.
  Host-tested; iOS and Android cannot drift on the wire.
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
  `SignForum*` RPCs; see the files' headers. The sign-in code lives under
  Settings, Support, "Sign in to the forum with a code" (the row under the
  forum link): main normalises the typed code (`forumLoginRequestFromCode`)
  and raises the same consent prompt as a deep link, on the one allowlisted
  broker, same-device. The prompt's outcome table is the fixture's, `expired`
  included, and it disarms Approve after a terminal refusal
  (`isTerminalForumLoginResult`) the way the Android prompt does.

## The bound approval: the code that finishes the sign-in

warren-connect binds a login to the browser that opened it
(`warren-connect/docs/FORUM-LOGIN-V2.md`, pinned by
`vectors/forum_login_v2.json`): `/sso` sets a `__Host-warren_login` cookie
in that browser, and the app's signed approval
(`{"login_version":2,"sid":...}`, built once by `warren_forum::login_body`
for every platform, the desktop daemon included) is answered with a
one-time six-digit code that the browser must present before it completes. A
relayed link, QR or typed code hands the code to the victim's app and leaves
the attacker's browser with a cookie and no code.

What the app does with the answer depends on how the sid arrived, one table
for every platform (`login.completion` in
`fixtures/client-rules/forum_outcomes.json`):

| how the sid arrived | screen | handoff |
|---|---|---|
| deep link without `xd` | "Finishing the sign-in in your browser", the code behind "The sign-in page is in another browser? Show the code" | `handoff_url` opened in the default browser at once, once |
| deep link with `xd=1` (the QR) | the code, large, with the warning never to read it out or send it | never, even one the provider sent |
| sign-in code typed under Settings | the code with the same warning, and "Finish in this device's browser" | on that button only |
| any of them, answer without `completion` | the approval as before: the browser completes on its own (a provider that predates the code, warren-connect v0.15.8 and older) | none |

The completion is validated before anything uses it: six ASCII digits, and a
handoff URL of exactly
`https://<connect host>/handoff#sid=<32 lowercase hex>&code=<the code>` (the
sid and the code ride in the fragment, which reaches no server log). A code of
another shape drops the whole completion; a handoff of another shape is
dropped on its own and the code kept. The Rust crate validates it for the
mobile envelope (`parse_login_completion`), the desktop main process validates
the HTTP answer itself (`parseForumLoginCompletion`), and Kotlin and Swift
re-check the envelope before they open anything.

The code and the handoff URL are live credentials until the session ends: they
live in memory for the screen that shows them, at most 300 s after the answer
(`code_lifetime_secs`), and are never logged, journaled, persisted, put in a
notification, copied to the clipboard or included in a problem report. Every
`Debug`, `toString` and `description` of the types that hold them prints
`<redacted>`, and the logs and the Android events journal carry the class
only (`approved-bound`). Where they live per platform:

- Desktop: `main/forum-login.ts` parses the answer and splits the plan
  (`planForumLoginApproval`): the handoff URL stays in main (opened at once, or
  kept in `PendingForumHandoff` for the button), the renderer gets the code and
  the screen through `forumLogin.approve` and asks main to open the kept
  handoff through `forumLogin.finishInBrowser`. The screen is the completion
  view of `ForumLoginPrompt`; its state is `prompt-state.ts`.
- Android: the FFI envelope carries an additive `completion` object;
  `WarrenForumLoginUseCase` decodes it into `ForumLoginCompletion`,
  `ForumLoginPromptState.complete` holds the screen and the handoffs (taken
  once, so a rotation opens nothing twice), and `ForumLoginPromptHost` shows
  the dialog and opens the handoff with an `ACTION_VIEW` intent.
- iOS: `WarrenAccountClient.forumLoginOutcome` decodes the same envelope into
  `WarrenForumLoginCompletion`, `WarrenForumCompletionSession` holds the
  handoffs, and `WarrenForumLoginCompletionView` is the sheet; the handoff
  opens with `UIApplication.open`.

Every other answer maps as before: `400 app_update_required` (the provider
refused an approval without `login_version`, which these clients never sign),
`429` (the wallet holds three approved sign-ins no browser completed) and
`502` (a database or gate read failed) are the generic failure, and the last
two leave the session waiting, so Approve stays armed for a retry.

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
   disconnecting and blocking defer the attempt with its own message and keep
   the prompt open for a retry (`login.deferred` in the journal, the state
   class only); connected, disconnected and failed proceed (a failed tunnel
   has released traffic, the TUN closed and the physical resolver back, and
   it is the state most worth reporting from): the mnemonic is read silently
   and handed to `WarrenJni.forumLogin`.
4. Rust reads `GET /v1/session/{sid}/status` once: a 404 is reported as
   `expired` without spending a signature; the `Date` header corrects a device
   clock outside the server's 60 s window (the 2026-08-18 class) before the
   signed POST.
5. The approved body carries the forum identity (`handle`, `notify_slot`);
   Android stores it (`ForumIdentityRepository`) and shows the "Forum name"
   on the account page. With a completion code the app shows the completion
   dialog of the section above; without one it moves to the background so the
   browser page, which only re-polls when visible, completes the login.

Every step writes its class to the events journal and the Rust log, so a
report filed afterwards explains a failure without a second round trip.

## The sign-in on iOS

The same three pieces as Android, in Swift over the shared crate:

- `ios/WarrenVPN/Classes/WarrenForumLogin.swift`: the link classification
  (`WarrenForumLinks.classify`, the fixture's rejection classes, logged by
  class only), the typed sign-in code (`normalizeSignInCode`), and the flow
  (consent prompt, the signed approval in Rust off the main thread, the result
  alert, the identity stored). The scheme the build answers and the connect
  host come from the Rust product table through
  `WarrenRustRuntime/WarrenProductAnchors.swift` (`warren_product_anchors()`),
  never from a literal: `ios/Configurations/ProductEnv.xcconfig` selects the
  environment for Xcode (`WARREN_PRODUCT_ENV`), `Info.plist` registers
  `$(WARREN_DEEP_LINK_SCHEME)`, and `build-rust-library.sh` compiles the Rust
  staticlib for the same environment. Until 2026-09-03 the plist spelled
  `warren` and the parser checked it, so a beta iOS install could never
  receive the beta broker's `warren-beta://` link.
- `warren-ios/src/warren_forum_ffi.rs`: the status preflight (the shared
  `classify_status_preflight`, the same table Android applies: a 404 is
  `expired` without a signature spent, the `Date` header corrects the device
  clock), the signed POST at the corrected time, the envelope of the shared
  crate with the identity.
- `WarrenRustRuntime/WarrenForumIdentityStore.swift`: the handle and the slot
  in the Keychain, device local, shown as "Forum name" on the account screen
  and erased with the wallet (`WarrenWalletKeychain.delete`).
- Settings, "Sign in to the forum with a code"
  (`WarrenForumSignInCodeView`), shown once a wallet exists: the same consent
  prompt as a deep link, on the allowlisted host, same device.

Readers of the fixtures: `WarrenForumLinkTests`, `WarrenForumLoginOutcomeTests`,
`WarrenProductAnchorsTests` (the `WarrenVPNCI` test plan). Not on iOS yet:
the tunnel-state preflight, the in-app report, the activity surface.

## The in-app report (desktop)

Settings, Support, "Report a problem" is the same form as the Android one
below, with the same field names, outcome table and copy (the locale entries
were taken from the Android strings). The pieces: the renderer view
(`components/views/report-problem/`, rules in
`features/report-problem/form-state.ts`, unit-tested off Electron), the main
process (`main/forum-report.ts`: the body fields, the outcome table replayed
from `forum_outcomes.json`, the POST under the body-sized deadline of
`uploadDeadlineMs`, the resend-without-logs class), and the daemon RPC
`SignForumReport`, which builds the body through `warren_forum::report_body`
(the crate the mobile clients sign with) and signs it with the daemon's own
signer; `mullvad-daemon`'s `forum_report_tests` replays the report requests of
`forum_login_v1.json` through that path. "View the logs" collects a report
main holds by id and opens it with the OS viewer (the attach prompt's path);
the send collects a fresh one, gzips it in main, and deletes the temp file
afterwards. The wallet address is passed to the collector as a redaction
string, as on Android. The management RPC and the gRPC client both carry a 16
MiB message cap for the report and attach bodies; tonic's 4 MiB default
refused an at-cap report before the daemon saw it.

Not on the desktop yet: the tunnel-state preflight and the clock preflight
against the connect host (the daemon stamps its own clock, as for the
sign-in), and the share-sheet export of the collected file.

## The in-app report (Android)

Settings, "Report a problem" mirrors the forum's "Report a bug" form (area,
what happened, steps, frequency). "Include technical logs" is on by default and
"View the logs" shows the exact file about to be sent. Send runs the same
tunnel-state preflight as the sign-in before collecting anything
(`report.deferred`, the form left intact), then collects a fresh report, gzips
it (12 MB cap, the broker's 16 M base64 characters translated to gzip bytes,
the same figure on the desktop), signs and POSTs it. The broker
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

The Rust side adds the live legs when the report is collected for a send
(`warren-jni/src/probes.rs`, each bounded to 6 s). A report collected for
"View the logs" reaches no host and records every probe key as `not-run`. The
protected leg, the API leg and the resolver run together; the default leg on
the connect host runs only after the protected one failed (`skipped`
otherwise), so the broker never sees the device address and the exit address
in the same instant.

| key | what it says |
|---|---|
| `probe-connect-protected` | the connect host's `/healthz` through the VpnService-protected socket: `ok-<ms>ms`, `http-<n>`, `connect-refused`, `connect-timeout`, `connect-unreachable`, `dns-failed`, `tls-failed`, `protect-refused`, `read-timeout` |
| `probe-connect-default` | the same route through the SDK's plain client, after a protected failure only: `ok-<ms>ms`, `http-<n>`, `connect-failed`, `read-timeout`, `io-failed`; `skipped` while the protected leg answered, `unavailable` when the client could not be built |
| `probe-dns-connect` | the system resolver on the connect host: `ok-<addresses>-<ms>ms`, `dns-failed`, `dns-timeout` |
| `probe-api` | the API's public `/v1/network` through the plain client, same classes as the default leg |
| `clock-offset`, `clock-offset-source` | server minus device seconds from the first dated answer, and which leg (`protected`, `default`) supplied it; `unknown` / `none` when nothing answered |

Read together with `tunnel-state`. A send only leaves while the tunnel is
`Connected`, `Disconnected` or `Failed` (the preflight defers the other states
before anything is collected), so those are the only states a submitted report
carries: a protected `ok` next to an API `read-timeout` while `Connected` is a
tunnel that passes nothing; `dns-failed` on the protected leg while
`Disconnected` or `Failed` is a resolver the device itself has lost; a large
`clock-offset` with `time-auto` off is the 2026-08-18 class. The deferred
states (`Connecting`, `Reconnecting`, `Disconnecting`, `Blocking`) appear in
the journal only (`login.deferred`, `report.deferred`, with the class), and in
a report shared by hand from "View the logs", whose probes read `not-run`.

The report POST rides a deadline sized to its body (20 s plus 10 s per MiB,
`warren_jni::forum::upload_deadline`), not the mint's 15 s: when it runs out
with logs attached the outcome is `upload-timeout`, and the screen offers the
resend without the logs.

When connect itself is unreachable, "View the logs" offers the system share
sheet: the redacted file leaves the device by any channel the user picks
(mail, a messenger, a file manager), so the report can still reach the forum
by hand. The `FileProvider` behind it serves the report directory only, under
an authority derived from the running package (each product environment has
its own application id).

## Forum activity on Android: the bell, the notification, the panel

The desktop rules apply unchanged (`forum-activity-monitor.ts`, ported rule
for rule as `ForumActivityMonitor` in `lib/repository`, with the same table
test): the badge comes only from the broadcast digest indexed by the wallet's
slot, a notification fires only on a rise seen while the app was watching, an
absent digest is unknown rather than zero, what the panel itself proved
overrides the digest until the document changes, and the setting removes the
whole header slot, lifebuoy included. Erasing the wallet clears the identity
(`ForumIdentityWalletBinding`), so the slot goes and the badge with it.

Where the pieces live: the digest verification, the anti-rollback high-water
mark and the freshness rule in `warren-jni/src/forum_digest.rs` (host-tested,
the daemon's `warren_forum_digest_updater` without the loop); the conditional
GET on the protected transport (`forumDigestFetch`); the panel read and the
mark-seen signed in the shared crate over their own paths and sent on the
protected transport (`forumNotifications`, `forumNotificationsSeen`), the rows
validated in `warren-forum` before they cross the FFI; the Kotlin monitor and
its wiring in `lib/repository` (`ForumActivityRepository`); the header slot in
`lib/ui/component` (`ForumHeaderSlot`); the panel screen and the "Forum
notifications" switch in the settings and notification features; the
low-importance `forum_activity` channel in `lib/push-notification`, whose
notification opens the panel through `KEY_OPEN_FORUM_ACTIVITY`.

**Poll model, decided 2026-09-03: foreground only.** `ForumDigestPoller` runs
inside `MainActivity`'s STARTED lifecycle: one fetch on every start (what the
app missed while away), then one a minute (the daemon's `CHECK_INTERVAL`, with
its 20 s to 45 s fast retry after a transport failure), cancelled the moment the
window is gone. No WorkManager period and no service wake-up: a background
cadence would make the app a periodic presence signal for a badge nobody is
looking at, WorkManager's 15 minute floor would make that badge lag anyway, and
the fetch on resume already catches up. The consequence to know: a reply that
arrives while the app is in the background raises the notification on the next
resume, not at once. A tunnel between states defers a fetch (the resolver is
the tunnel's, or absent under the kill switch) and counts as unreachable for the
retry cadence.

## Reading a failed attempt

- Device: `adb logcat -s WarrenJni:V` for the live run;
  `files/rust_logs/warren.log`, `files/android_app_logs/warren-events.log`
  for the history; both are inside every in-app report.
- Forum host: `docker logs warren-warren-connect-1 | grep -E "forum login|in-app report"`,
  and the connect edge log for a POST that never arrived.

## The contract, pinned

Two files keep the four implementations (Rust, Kotlin, TypeScript, Swift)
from drifting, each replayed by every platform's own unit tests:

- `vectors/forum_login_v1.json` (the warren-vectors submodule): the exact
  signed bytes of a report and of an attach, the login form that predates the
  completion code, and the broker's exact answer per outcome; synthetic host
  and key. Replayed by `warren-forum/src/forum_login_vector_tests.rs` through
  the nonce-taking builders (`build_signed_report_request_with_nonce`,
  `build_signed_attach_request_with_nonce`), and by warren-connect on the
  other side of the wire.
- `vectors/forum_login_v2.json`: the bound login the app signs, its answers
  with the completion code and the handoff URL, and the browser half.
  Replayed by `warren-forum/src/forum_login_v2_vector_tests.rs` (the request
  through `build_signed_request_with_nonce`, every login answer, the handoff
  validation on a foreign host, a query string and another code) and by the
  daemon's `the_daemon_signs_the_vectors_bound_login_byte_for_byte`.
- `fixtures/client-rules/` (this repo): the deep-link classes per scheme,
  the outcome table with the FFI envelope of every outcome, the product
  anchors per environment. Readers, schema and the skip lists still in force
  are in its README; the skip lists are the measure of the remaining
  desktop and iOS divergence.
