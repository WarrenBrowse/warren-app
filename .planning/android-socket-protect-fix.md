# Android tunnel socket protection (C2 blocker) : 2026-06-12

Fixes the top Android blocker from the 2026-06-12 audit: the Warren tunnel's
own Quinn UDP socket was never protected, so with the TUN capturing
`0.0.0.0/0` the handshake packets looped back into the TUN and the tunnel
could not pass traffic on a real device.

## What was changed

Cross-repo (the socket is born inside quinn, in warren-core; the
`VpnService.protect` call can only be made from the Android FFI layer).

### warren-core (`../warren-core`, sibling repo : UNCOMMITTED, needs commit + re-pin)
- `crates/warren-tunnel/src/socket_protect.rs` (new, `#[cfg(target_os = "android")]`):
  a process-global `SocketProtector = Arc<dyn Fn(RawFd) -> bool + Send + Sync>`
  with `set_protector` / `protect`. `protect` returns `true` when no protector
  is registered (CLI client + tests are unaffected; only the VpnService host
  installs one).
- `crates/warren-tunnel/src/lib.rs`: `#[cfg(target_os = "android")] pub mod socket_protect;`
- `crates/warren-tunnel/src/client.rs` `bind_client_endpoint`: on Android, bind
  the `UdpSocket` ourselves, call `socket_protect::protect(fd)` BEFORE building
  the quinn `Endpoint` (so it is protected before any packet egresses), then
  `Endpoint::new(...)`. Every other platform keeps `Endpoint::client` verbatim.
  This is the single chokepoint for `connect` / `connect_multi` / handshakes.

### warren-app
- `warren-jni/src/android_jni.rs`: `connectTunnel` gains a `vpn_service: JObject`
  param. New `register_socket_protector` captures the `JavaVM` + a global ref to
  the `VpnService` and installs a protector that attaches the calling tokio
  thread to the JVM and calls `VpnService.protect(int)` on the fd. Registered
  before the session spawns.
- `android/.../jni/WarrenJni.kt`: `connectTunnel(vpnService, tunFd, mnemonic, configJson)`
  + `import android.net.VpnService`.
- `android/.../service/WarrenQuinnAdapter.kt`: passes `vpnService` (already held)
  to the call.

With protect in place the existing "establish TUN before dial" order is fine:
the protected socket bypasses the VPN route regardless.

## Verified
- Host (`cargo check -p talpid-warren-tunnel`): green (desktop path untouched).
- `aarch64-linux-android` (`cargo check`/`clippy -p warren-jni --features tunnel`,
  NDK 27 clang wired): green, zero warnings. This compiles BOTH the JNI code and
  the warren-core android path (socket_protect + protected endpoint).

## NOT done / still required to land
1. Commit the warren-core change in `../warren-core`, then update
   `.warren-core-version` and commit the warren-app side together (CI uses the
   pinned SHA, not the local checkout, so they must land as one unit).
2. Gradle/Kotlin build (`libwarren_jni.so` via cargo-ndk) to confirm the Kotlin
   side compiles end-to-end.
3. ON-DEVICE leak test: one real connect to a prod exit, confirm traffic flows
   and a DNS/IP leak test passes both connected and in the failed-but-blocking
   state.

## Other Android blockers ALSO fixed this session (warren-app only, no warren-core dep)

### Kill switch: fail-closed on every unexpected drop (`WarrenQuinnAdapter.onSessionDown`)
Previously an unexpected drop with the kill switch OFF released traffic to the
physical network (leak). Now ANY non-user drop fails closed first (blackhole +
retry) regardless of the toggle; the toggle only decides the resting state once
recovery has clearly failed (parked when on, released when off). Matches the
Mullvad model. No default flip, so no settings/test churn. Renamed
`scheduleLockdownReconnect` -> `scheduleDropReconnect` (now runs for all drops).

### Handover: no interface-less window (`WarrenQuinnAdapter.scheduleHandoverReconnect`)
Previously torn down the tunnel then slept 15s with NO TUN, leaking during the
gap. Now establishes the blackhole interface BEFORE teardown (establish()
atomically replaces the live interface), so traffic stays captured across the
whole reconnect gap. The 15s grace stays (leak-safe now; UX blip is secondary).

### Manifest: app is launchable (`android/app/src/main/AndroidManifest.xml`)
MainActivity had no MAIN/LAUNCHER filter, `android:exported="false"`, and a bogus
`android:targetActivity="com.warrenbrowse.vpn.ui.MainActivity"` (invalid on
`<activity>`, and `.ui.MainActivity` does not exist; the real class is
`.app.MainActivity`). Removed the bad attribute, set `exported="true"`, added a
MAIN/LAUNCHER (+ LEANBACK_LAUNCHER) intent-filter. The app now has a home-screen
entry point.

### Fallback relay: FLAGGED, not fixed (`android_jni.rs` FALLBACK_RELAYS_JSON)
`warren-exit-1.warren.brown:443` is a hostname (`tunnel.rs` parses exit_endpoint
as `SocketAddr` -> never connectable) AND `exit_pubkey_hex` is only 16 bytes
(32 hex) where an Ed25519 pubkey is 32 bytes (64 hex) AND the geo is wrong. It is
a non-functional placeholder. Making it real needs verified prod exit_id +
32-byte pubkey + IP:port from ops, so it was left untouched (no guessed crypto).
Only hit in degraded mode (signed /v1/exits fetch failed); the normal path uses
real IP:port endpoints and works with protect().

## LANDING BLOCKER (cross-repo pin sequencing, user decision)
warren-app pin is `a7bd85aa` (0.3.11). warren-core HEAD is `ca2b149` (0.3.12),
which includes the wire-v2 multihop change marked "redeploy-together". The
protect fix needs warren-core `socket_protect` (only past HEAD), so pinning it in
forces wire-v2 into warren-app. Safe ONLY once prod exits run wire-v2, else
handshake mismatch. The warren-core change is android-cfg only (zero exit-binary
impact), so no version bump is strictly required for it.

Nothing is committed. To land once exits are wire-v2-ready:
1. Commit warren-core `socket_protect` (3 files) on main.
2. Bump `.warren-core-version` to that SHA.
3. Commit warren-app (warren-jni + Kotlin + manifest + pin) together.
4. Gradle build + on-device leak test.
