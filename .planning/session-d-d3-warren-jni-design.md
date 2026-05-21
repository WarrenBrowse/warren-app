# D.3 — `warren-jni` crate design

Companion document to the D.3 commit. Captures the architectural shift away
from upstream `mullvad-jni`, the deferred work items, and the Android-target
build details needed for D.4-D.7.

---

## 1. Why the shape changed

Upstream `mullvad-jni` is a thin JNI veneer around a **full** `mullvad-daemon`
that runs in-process: it spins up a tokio runtime, registers an IPC socket,
serves the gRPC management interface, holds the relay selector, owns the WG
tunnel state machine, and orchestrates reconnect / account / split-tunnel
logic. That model assumes you also bundle the desktop CLI and the
management-interface gRPC client on top.

Warren Android does **not** need any of that on-device:

- No CLI on phones (no shell access for end-users).
- No gRPC management interface on phones (the only consumer is the app
  process itself; calling Kotlin <-> Rust over loopback gRPC just to traverse
  a JNI boundary is pure overhead).
- No full account-aware state machine (Warren's auth is non-custodial: the
  wallet lives in Android Keystore + EncryptedSharedPreferences; signing is
  per-request via `warren-identity::auth`).
- No WireGuard userspace adapter (Warren uses Quinn pure-Rust + the upstream
  `tunnel-obfuscation` is replaced by M4.0 framing inside `warren-tunnel`).

So D.3 drops `mullvad-daemon`, `mullvad-api`, `mullvad-problem-report` from
`warren-jni` deps. The new surface is a small set of stateless / single-owner
primitives that Kotlin orchestrates from `WarrenVpnService`.

## 2. Final crate layout

```
warren-jni/
  Cargo.toml          # warren-* path-deps + jnix + tokio + thiserror
  src/lib.rs          # JNI exports, ACTIVE_TUNNEL slot, shared RUNTIME
```

`src/api.rs`, `src/classes.rs`, `src/problem_report.rs` were dropped: they
all referenced `mullvad-api` / `mullvad-daemon` / `mullvad-problem-report`.
The Java class preload list (`classes.rs`) gets re-introduced in D.4 once the
Warren-side talpid model types stabilise (or, more likely, are replaced by
serde-over-JSON config blobs to avoid the `jnix` class-handle dance).

## 3. JNI export surface (target)

All exports use the `Java_com_warrenbrowse_vpn_jni_WarrenJni_<name>`
mangling. The Kotlin facade lives in
`android/app/src/main/kotlin/com/warrenbrowse/vpn/jni/WarrenJni.kt`.

| Rust export | Kotlin signature | Lands in |
|---|---|---|
| `initLogger` | `fun initLogger(filesDirectory: String)` | D.3 (stub OK) |
| `generateMnemonic` | `fun generateMnemonic(): String` | D.5 |
| `importMnemonic` | `fun importMnemonic(mnemonic: String): ByteArray` | D.5 |
| `signRequest` | `fun signRequest(canonicalMessage: ByteArray): ByteArray` | D.5 |
| `connectTunnel` | `fun connectTunnel(tunFd: Int, configJson: String): Int` | D.4 |
| `disconnectTunnel` | `fun disconnectTunnel()` | D.4 |
| `getTunnelStatus` | `fun getTunnelStatus(): Int` | D.4 |
| `listRelays` (planned) | `fun listRelays(): String` (JSON) | D.6 |
| `enableNatPmp` (planned) | `fun enableNatPmp(enable: Boolean)` | D.6 |

`tunFd` is the raw fd from `VpnService.Builder.establish()`, duped on the
Kotlin side so the Rust side owns the lifetime.

## 4. Workspace + build wiring

- Root `Cargo.toml`: `mullvad-jni` removed from members, `warren-jni` added.
- `android/app/build.gradle.kts` `cargo {}` block: `libname = "warren-jni"`,
  `targetIncludes = ["libwarren_jni.so"]`, `--package=warren-jni`.
- `local.properties` (gitignored): seeds `sdk.dir` and
  `ndk.dir=/Users/poka/Library/Android/sdk/ndk/29.0.13113456`.

Build verification on macOS host (no Android Studio needed):

```
ANDROID_HOME=$HOME/Library/Android/sdk \
ANDROID_NDK_HOME=$HOME/Library/Android/sdk/ndk/29.0.13113456 \
CC_aarch64_linux_android=$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/darwin-x86_64/bin/aarch64-linux-android26-clang \
AR_aarch64_linux_android=$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/darwin-x86_64/bin/llvm-ar \
CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER=$CC_aarch64_linux_android \
cargo check -p warren-jni --target aarch64-linux-android
```

Same pattern for `armv7-linux-androideabi` (uses
`armv7a-linux-androideabi26-clang`) and `x86_64-linux-android`. All three
targets compile cleanly with the default feature set as of this commit.

## 5. The Android TUN gap (deferred to D.4)

The `warren-tunnel` crate uses `tun-rs::DeviceBuilder` to build the in-Rust
TUN endpoint. `tun-rs` 2.8 exposes `DeviceBuilder` only on Linux, macOS,
Windows, FreeBSD, OpenBSD, and NetBSD - **not Android**. The kernel-side
TUN/TAP path on Android is unreachable to a non-root userspace process;
instead the system VpnService hands the app a TUN file descriptor.

Two viable strategies, both cross-repo work in `warren-core`:

1. **Direct fd wrap (recommended).** Add a thin
   `warren_tunnel::PacketDevice` impl backed by a raw OS fd
   (`std::os::fd::OwnedFd` + tokio `AsyncFd`). The fd is duped from Kotlin
   via JNI, handed into `warren-jni::connectTunnel`, then into
   `warren_tunnel::ClientConfig::with_packet_device(...)`. No tun-rs
   involvement on Android. Side benefits: same impl can serve iOS once
   C.4 needs it.

2. **Patch tun-rs.** Submit a `target_os = "android"` arm upstream. Bigger
   blast radius (tun-rs has its own backend matrix) and lands on tun-rs's
   schedule, not ours.

Until either lands, `warren-jni`'s `tunnel` Cargo feature stays off by
default. The skeleton ships the JNI symbols (so Kotlin can call them) but
the implementation slot is empty.

## 6. Open D.4 work items (cross-repo)

- `warren-core`: add `PacketDevice::from_fd(fd: OwnedFd)` constructor +
  feature-gate the `tun-rs` import in `warren-tunnel/src/real_tun.rs` to
  `not(target_os = "android")`.
- `warren-app/warren-jni`: enable `features = ["tunnel"]` in the Cargo.toml
  consumed by the Android build, write the real `connectTunnel` body
  (deserialize `WarrenTunnelConfig`, build `warren_tunnel::ClientConfig`,
  spawn the Quinn pump on the shared runtime, hand the tun fd in).
- `warren-app/android`: drop `WarrenDaemon.kt` and the `lib/talpid` module's
  Mullvad-flavoured `TalpidVpnService` once `WarrenVpnService` is wired
  directly against `WarrenJni`.

## 7. What is not in scope (will surface naturally)

- `lib/talpid` rebrand. The module still ships its upstream
  `net.mullvad.talpid.*` namespace + Mullvad-style TUN config marshalling.
  It will either be deleted wholesale during D.4 (preferred) or rebranded
  in place if pieces of the `TalpidVpnService` shape survive as the Warren
  Quinn carrier.
- `MullvadApi` / `MullvadApiTest` e2e tests. They drive the upstream Mullvad
  backend - replaced by Warren-API-backed tests during D.6.
- Bundled relay list. Mullvad bakes a relay JSON into `dist-assets/relays/`;
  Warren fetches relays from `warren-api-client` at runtime. The Gradle
  `sourceSets.main.assets.directories` entry pointing at that dir can stay
  for now (the file is just bytes) but should be removed when the
  warren-api-backed relay listing is integrated in D.6.
