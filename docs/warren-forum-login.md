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
- **Android + iOS: BLOCKED on a prerequisite (2026-07-03 finding).** These
  apps have NOT yet integrated the Warren gRPC management service at all: no
  `getWarrenMnemonic`/`setWarrenApiUrl`/`WarrenStatus` calls exist, the
  repositories are stubbed (`managementService: Any? = null`), and there is no
  generated Kotlin/Swift ManagementService stub. Forum login cannot be built
  before the mobile apps wire the Warren management service (a separate epic).
  The daemon-side `SignForumLogin` RPC is already shipped and ready for that
  work. When the mobile Warren integration lands, implement as below.

## Android (Kotlin) - once the Warren management service is wired

- Add an intent filter for the deep link in `AndroidManifest.xml`:
  `<data android:scheme="warren" android:host="forum-login"/>` on a
  lightweight Activity.
- The Activity extracts `sid`+`host`, validates the host allowlist + sid shape,
  calls `signForumLogin(sid)` over the management-interface, POSTs the headers
  to the connect host, finishes silently (or shows a toast). "Community" button
  opens the forum URL in a Custom Tab.

## iOS (Swift) - once the Warren management service is wired

- Add `warren` to `CFBundleURLSchemes` and handle it in
  `application(_:open:options:)` / the SwiftUI `.onOpenURL`.
- Parse+validate `sid`+`host`, call `signForumLogin` over the management
  bridge, POST to the connect host, present a toast. "Community" opens the
  forum in `SFSafariViewController`.

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
