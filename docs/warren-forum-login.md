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

## Android (Kotlin) - IMPLEMENTED (2026-07-06), device test pending

The full Kotlin glue is landed on branch `android-forum-login` and verified as
far as headless allows: `:app:compileProdDebugKotlin` is green (with
`allWarningsAsErrors`) and 9 JVM unit tests pass
(`:app:testProdDebugUnitTest --tests "com.warrenbrowse.vpn.app.forum.*"`). DI is
Koin, logging is Kermit, JSON parsing is kotlinx.serialization. What shipped:

1. **Intent filter** (`AndroidManifest.xml`): a third `<intent-filter>` on
   `MainActivity` (`ACTION_VIEW` + `DEFAULT` + `BROWSABLE`,
   `<data android:scheme="warren" android:host="forum-login"/>`), the app's first
   deep link.
2. **Deep-link parse** (`app/forum/ForumLoginLink.kt`): a PURE `parseForumLoginLink(
   rawUrl): ForumLoginLink?` (java.net.URI, no Android `Uri`, so it is JVM
   unit-testable) that enforces scheme `warren`, action `forum-login`, sid
   `^[0-9a-f]{32}$`, host allowlist `connect.warrenbrowse.com`. Wired into
   `MainActivity.handleIntent` as an `Intent.ACTION_VIEW` arm feeding
   `ForumLoginController`; the existing `addOnNewIntentListener` callbackFlow
   already delivers both cold-start and warm links (singleInstance).
   Unit-tested (`ForumLoginLinkTest`, mirrors the desktop spec).
3. **Consent** (`app/forum/ForumLoginPromptHost.kt`): a Compose `AlertDialog`
   overlay added at `MainActivity` `setContent` alongside `WarrenApp`, observing
   `ForumLoginController.pending`. Approve = `PrimaryButton` (a plain
   Material3 dialog, not the destructive-framed `NegativeConfirmationDialog`);
   never signs silently. A `busy` guard keeps the composable in composition
   until the call returns so the coroutine is not cancelled mid-flight.
4. **Sign + POST** (`app/forum/WarrenForumLoginUseCase.kt`, Koin `single`,
   pattern of `WarrenSubscriptionUseCase`): reads the mnemonic silently and, on
   `Dispatchers.IO`, `mnemonic.use { WarrenJni.forumLogin(it.phrase, sid, host) }`
   (Rust signs + POSTs). The pure `parseForumLoginOutcome(json)` maps the
   `{"ok":...}` envelope to `Approved` / `SubscriptionRequired` / `Failure`
   (unit-tested, `ForumLoginOutcomeTest`); the prompt toasts the result and (todo
   on device) opens the forum in a Custom Tab on success. No Kotlin HTTP client,
   no nonce/timestamp/header plumbing (all in Rust).

Remaining, genuinely device-gated:
- **Device round-trip test**: browser invokes `warren://forum-login` -> OS routes
  to the app -> consent -> sign -> the browser approval page completes and the
  user lands logged in under an opaque handle (subscriber group applied for a
  subscribed wallet). Needs a real device/emulator + the live connect provider.
- **Custom Tab open-on-success + a "Community" entry point** (open
  `https://forum.warrenbrowse.com`): trivial UI, deferred to the device session.
- **Cancel notify** (`POST /v1/session/<sid>/cancel`): optional; the session
  expires in 10 min regardless. If added, do it via a small Rust JNI helper to
  keep the no-Kotlin-HTTP invariant, not an OkHttp dependency.

## iOS (Swift) - IMPLEMENTED (2026-07-06), device test pending

Landed on branch `android-forum-login` and, like Android, verified as far as
headless allows: the Rust FFI is host-tested + iOS-target compiled, and the
whole `WarrenVPN` app builds for the iOS simulator (Xcode 26.4 is present after
all; only the on-device round-trip is gated). What shipped:

1. **Rust FFI** (`warren-ios`): a new host-tested `forum` module (8 host tests:
   allowlist, sid shape, wire assembly, nonce, status mapping, envelope) that,
   unlike Android, signs directly via `WarrenIdentity::from_seed(seed)
   .sign_request(...)` (no `sign_forum_login` port needed, the iOS house pattern
   passes the 32-byte seed). The iOS-gated `warren_forum_ffi.rs` exports
   `char *warren_forum_login(const uint8_t *seed, const char *sid, const char
   *host)` (auto-added to `warren_rust_runtime.h` by the cbindgen `build.rs`),
   executing the POST via `ReqwestTransport::execute` on the shared iOS runtime,
   returning the same `{"ok":...}` envelope as Android.
2. **Swift** (existing files only, no `.pbxproj` surgery): `WarrenForumLoginOutcome`
   + `WarrenAccountClient.forumLogin(seed:sid:host:)` (reuses the account
   client's `parseEnvelope`), a `warren://forum-login` `CFBundleURLTypes` entry in
   `Info.plist`, and `SceneDelegate` handling (`scene(_:openURLContexts:)` + the
   cold-launch `connectionOptions.urlContexts` path, a `parseForumLogin` guard,
   a `UIAlertController` consent prompt, silent seed load via
   `WarrenWalletKeychain.load()` + `WarrenWallet.fromMnemonic`, and a result
   alert). Rust re-validates sid+host, so the Swift parse is a fail-fast.

Verified on this host: `cargo test -p warren-ios` (8 forum tests green),
`cargo build -p warren-ios --target aarch64-apple-ios-sim` clean, and
`xcodebuild -scheme WarrenVPN -destination 'generic/platform=iOS Simulator'
CODE_SIGNING_ALLOWED=NO` exits 0 (the `ios/Configurations/*.xcconfig` are
gitignored per-dev files, generated from the `.template`s for the compile check).

Remaining, device-gated only (same as Android): the on-device deep-link
round-trip, open-forum-on-success (`SFSafariViewController`) + a "Community"
entry point, and the optional cancel-notify.

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
