# Warren forum login: app integration spec

Wire the Warren app so a user signs into the community forum
(`forum.warrenbrowse.com`) with their existing wallet key, no email, no
password. Server side is live and E2E-proven (warren-core
`docs/55-FORUM-DISCOURSE-SUPPORT.md`); this is the last-mile app glue.

Desktop is Electron/TypeScript/React, Android is Kotlin, iOS is Swift. Each
needs the deep-link handler + a "Community" entry; all three call one new
daemon gRPC method. Every step below must be built AND device-tested per
platform (a deep-link GUI flow cannot be verified without a real device).

## The flow (already working server side)

```
1. User taps "Community" -> app opens https://forum.warrenbrowse.com in the
   system browser.
2. User clicks "Log In" on the forum -> Discourse 302s to
   connect.warrenbrowse.com/sso (DiscourseConnect), which shows an approval
   page and a deep link:  warren://forum-login?sid=<32hex>&host=connect.warrenbrowse.com
3. The browser invokes the deep link -> the OS hands it to the Warren app.
4. The app signs POST https://<host>/v1/forum/login with body {"sid":"<sid>"}
   using the SAME canonical X-Warren-* signature it already uses for the API,
   and sends it.
5. The browser's approval page (polling) sees "approved" and completes the
   DiscourseConnect redirect: the user is logged in under an opaque handle.
```

The signature is byte-identical to the daemon's existing API auth. Reference
implementation proven end to end: `warren-connect/examples/e2e_sign.rs`.

## Daemon: one new gRPC method (the only new Rust)

No new crypto: reuse `mullvad_api::warren_auth::WarrenAuthSigner`, which the
daemon already holds. It exposes exactly the primitive needed:

```rust
// mullvad-api/src/warren_auth.rs
signer.sign_request("POST", "/v1/forum/login", body) -> WarrenAuthHeaders
```

Add a management-interface RPC:

```proto
// mullvad-management-interface/proto/management_interface.proto
rpc ForumLogin(ForumLoginRequest) returns (google.protobuf.Empty);
message ForumLoginRequest {
  string sid  = 1;  // opaque session id from the deep link
  string host = 2;  // connect host from the deep link (validate allowlist)
}
```

Daemon handler:
1. Validate `host` against an allowlist (`connect.warrenbrowse.com`; plus a
   dev host if a debug build) so a hostile deep link cannot point the signed
   request at an attacker.
2. `body = format!("{{\"sid\":\"{sid}\"}}")` with `sid` validated as 32 lowercase
   hex chars (reject otherwise: it is attacker-influenced).
3. `let headers = signer.sign_request("POST", "/v1/forum/login", body.as_bytes());`
4. POST `https://{host}/v1/forum/login` with the 4 `X-Warren-*` headers +
   `Content-Type: application/json` + `body`. A plain `reqwest`/hyper client
   is fine here (public endpoint; this call does not need the censorship
   transport, though routing it through the daemon's outbound path is
   acceptable too).
5. Map a non-2xx to a typed error surfaced to the UI as a toast.

The daemon does the HTTP so the wallet key never leaves the daemon process
(the renderer/native UI only passes `sid`+`host`).

## Desktop (Electron)

- Register the `warren://` scheme: `app.setAsDefaultProtocolClient('warren')`
  and handle `open-url` (macOS) / `second-instance` argv (Windows/Linux) in
  the main process. Parse `warren://forum-login?sid=..&host=..`.
- On receipt, call the new `ForumLogin` gRPC via the existing
  management-interface client, then show a success/failure toast.
- "Community" entry (e.g. in the main menu / settings): open
  `https://forum.warrenbrowse.com` with `shell.openExternal`. While the
  pre-launch `basic_auth` gate is up, prefill it or instruct the user
  (user `warren`).

## Status

- **Desktop (Electron): DONE.** Deep-link handler (`src/main/forum-login.ts`),
  `warren://` scheme registration, `SignForumLogin` daemon RPC, and the
  "Community forum" button (Support view) are shipped and verified at compile
  level (tsc/eslint/vitest + daemon cargo check/clippy). Device click-through:
  macOS verified 2026-07-03 (app running); Windows and Linux remain, in both
  states (app running AND app closed, see cold start below).

### Desktop delivery paths and cold start (all three OS)

How the OS hands the link to the app, and what makes it robust:

- **Scheme registration.** macOS: `CFBundleURLTypes` written by
  electron-builder from the `protocols` config. Linux: the generated
  `.desktop` carries `MimeType=x-scheme-handler/warren;` and `Exec=... %U`
  (the launcher script forwards `"$@"` to `warren-gui`); distro
  `desktop-file-utils` triggers rebuild `mimeinfo.cache` on install. Windows:
  electron-builder NSIS does NOT register URL protocols; registration happens
  at runtime (`app.setAsDefaultProtocolClient`, HKCU) on every app start, so
  the scheme exists from the first launch onward.
- **Delivery.** macOS: `open-url` event. Windows/Linux: argv of the second
  instance (app running) or `process.argv` at startup (app closed).
- **Cold start buffering.** A deep link that launches the app arrives before
  the renderer exists, so the `forumLogin.request` IPC push would be lost.
  The main process buffers the latest unanswered request
  (`PendingForumLogin`, 10 min TTL matching the server session lifetime) and
  the consent prompt fetches it on mount (`forumLogin.getPending`). The
  buffer also survives a window close/reopen until the user approves or
  cancels; only a transient submit error keeps it for retry.
- **Android + iOS: the 2026-07-03 "blocked on a gRPC management-service
  epic" finding is CORRECTED (2026-07-05).** The premise was wrong: Android
  has no `mullvad-daemon` and no gRPC at all - it talks to Rust through the
  JNI bridge `libwarren_jni.so`, and the exact wallet-signing capability
  `SignForumLogin` wraps is ALREADY exposed and exercised there
  (`WarrenJni.signCanonicalRequest`, used today for signed subscription /
  voucher requests). So forum login is a SMALL additive slice, not blocked;
  you mirror the desktop *behavior*, not its gRPC *transport* (the desktop
  `SignForumLogin` gRPC RPC will never be reachable from Android and does not
  need to be). Security note: the boundary differs - on desktop the key
  never leaves the daemon; on Android the mnemonic is unlocked from the
  Keystore and passed into JNI per call (the existing model for
  subscription/voucher/tunnel signing), so forum login inherits that.

  **Rust side DONE (2026-07-05/06, branch `android-forum-login`):** the JNI
  layer signs AND sends, so ONLY `sid`+`host` cross the boundary (the faithful
  Android mirror of the desktop, where the renderer passes only sid+host and the
  daemon does the signing + POST). A key map finding drove this: there is NO
  general-purpose HTTP client anywhere in the Kotlin code, every other signed
  call (`getSubscription`, `redeemVoucher`, `sendProblemReport`) signs and POSTs
  inside Rust via `warren-api` and returns only result JSON. So forum login does
  the same rather than introducing OkHttp with no house pattern.
  - `warren-jni::forum` (host-compiled, 7 host tests): `build_signed_request(
    mnemonic, sid, host)` validates the connect-host allowlist, generates a
    fresh timestamp + random 16-byte nonce, reuses `wallet::sign_forum_login`
    (still host-tested for wire parity) for the canonical `{"sid":"<sid>"}` body
    + four `X-Warren-*` headers, and returns the exact URL/headers/body to POST;
    `outcome_for_status` (2xx approved, 403 subscription-required, else failed)
    and `envelope` map to the Kotlin JSON. All fully unit-tested off-device.
  - `warren-jni::android_jni::forum_login` (Android-only) executes that request
    on the shared tokio runtime through the reqwest transport `warren-api`
    already bundles, mapping the HTTP status via `outcome_for_status`.
  - JNI export `WarrenJni.forumLogin(mnemonic, sid, host): String` returning
    `{"ok":true}` / `{"ok":false,"error":"subscription-required"}` /
    `{"ok":false,"error":"error"}`. Blocks on the POST (call off the main
    thread). The mnemonic, sid, signature and nonce are never logged.

## Android (Kotlin) - remaining app glue (unblocked)

All additive; needs the Gradle/NDK build loop (build on the macstudio runner
or a local NDK-configured checkout, then a device test). File anchors below are
from the 2026-07-06 code map. DI is Koin (not Hilt); logging is Kermit
(`co.touchlab.kermit.Logger`); JSON parsing is kotlinx.serialization.

1. **Intent filter** in `android/app/src/main/AndroidManifest.xml` on
   `MainActivity` (a third `<intent-filter>` alongside MAIN/LAUNCHER, activity
   block at `AndroidManifest.xml:66-86`; the activity is already
   `exported="true"`, `launchMode="singleInstance"`): `ACTION_VIEW` +
   `CATEGORY_DEFAULT` + `CATEGORY_BROWSABLE` with
   `<data android:scheme="warren" android:host="forum-login"/>`. There is no
   existing deep-link filter in the app, this is the first.
2. **Deep-link branch** in `MainActivity.handleIntent` (`MainActivity.kt:138`,
   the `when` whose `else` drops unknown actions to `Logger.w(...)`): add an
   `Intent.ACTION_VIEW` arm that reads `intent.data`, extracts `sid`+`host`, and
   fails fast on a non-allowlisted host or a `sid` not matching `^[0-9a-f]{32}$`
   (Rust re-validates). Because `launchMode=singleInstance`, both cold-start and
   warm links already flow through `handleIntent` via the `addOnNewIntentListener`
   `callbackFlow` collected in `onCreate` (`MainActivity.kt:160-169`), no
   `onNewIntent` override needed.
3. **Consent** prompt (mirror `ForumLoginPrompt.tsx`; never sign silently): the
   app is 100% Compose, so hoist a `StateFlow<ForumLoginRequest?>` that
   `MainActivity` sets from `handleIntent` and a composable observes, showing a
   reusable `NegativeConfirmationDialog`
   (`lib/ui/component/.../dialog/NegativeConfirmationDialog.kt`) titled "Sign in
   to the Warren community forum?" with approve/cancel (copy from the desktop
   prompt: signs a one-time challenge, no email/password, anonymous handle).
4. **Sign + POST** on approve: a `WarrenForumLoginUseCase` (Koin `single`,
   pattern of `WarrenSubscriptionUseCase`) that reads the mnemonic silently
   (`walletRepository.readMnemonic()`, no biometric gate today) and, on
   `Dispatchers.IO`, `mnemonic.use { WarrenJni.forumLogin(it.phrase, sid, host) }`
   (the `.use{}` zeroes the CharArray). Parse the `{"ok":...}` with
   kotlinx.serialization: `ok:true` -> success toast then open the forum in a
   Custom Tab; `error:"subscription-required"` -> "subscription required"
   message; else -> generic failure. No Kotlin HTTP client, no nonce/timestamp
   handling, no header plumbing (all in Rust now).
5. **Cancel**: notify the provider so the waiting browser page unblocks
   (`POST https://<host>/v1/session/<sid>/cancel`, best-effort). This is the one
   unsigned call; add a tiny `WarrenJni.forumLoginCancel(host, sid)` (or fold it
   into the use case via a Rust helper) rather than a Kotlin HTTP client, to
   keep the no-Kotlin-HTTP invariant. Optional for a first cut (the session
   expires in 10 min regardless).

## iOS (Swift) - later, more gated (needs Xcode)

Same shape as Android: the Rust `warren-jni::forum` logic is reusable; add an
equivalent FFI entry in the `warren-ios` crate (`forum_login(mnemonic, sid,
host) -> {"ok":...}`, ideally by lifting `forum.rs` to a shared crate so it is
not duplicated). Swift side: add `warren` to `CFBundleURLSchemes`, handle it in
`.onOpenURL`, validate `sid`+`host`, call the FFI, present a toast. "Community"
opens the forum in `SFSafariViewController`. Gated on an Xcode build + Apple
signing (not available on the current host).

## Security checklist (all platforms)

- Validate `host` against a hard allowlist before signing anything.
- Validate `sid` shape (32 lowercase hex) before building the body.
- The signed request is single-use server side (nonce anti-replay) and the
  session expires in 10 min; no additional client-side replay guard needed.
- Never log `sid`, the pubkey, or the signature in the clear (Warren no-log).
- The deep link carries no secret (only an opaque session id), so an
  intercepted link cannot log anyone in without the wallet key held by the
  daemon.

## Definition of done (per platform)

Built, `warren://forum-login` registered, deep link round-trips to a real
`ForumLogin` daemon call, and a device test shows the browser landing logged
in under an opaque handle with the subscriber group applied for a subscribed
wallet. Track in warren-core `docs/55-GOAL-STATE.md`.
