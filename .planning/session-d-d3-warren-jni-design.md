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

## 5. The Android TUN gap (resolved)

**Landed in warren-core `d59fd15` (2026-05-21).** A new
`warren_tunnel::AndroidTun` impl of `PacketDevice` wraps the
VpnService-provided fd via `tokio::io::unix::AsyncFd` + safe POSIX wrappers
from `nix` (the workspace `unsafe_code = "forbid"` lint forbids raw
`libc`). The fd lifetime is managed by `OwnedFd`; non-blocking is set via
`fcntl(F_SETFL | O_NONBLOCK)` once at construction time. No virtio_net_hdr
/ GSO offload: Android's TUN driver does not expose those.

Background (kept for reference): `warren-tunnel` originally used
`tun-rs::DeviceBuilder` to build the in-Rust TUN endpoint. `tun-rs` 2.8
exposes `DeviceBuilder` only on Linux, macOS, Windows, FreeBSD, OpenBSD,
and NetBSD - **not Android**. The kernel-side TUN/TAP path on Android is
unreachable to a non-root userspace process; instead the system
VpnService hands the app a TUN file descriptor. We chose option (1)
"direct fd wrap" rather than patching tun-rs upstream because the surface
is small and the iOS side (C.4) needs an equivalent
`PacketTunnelProvider`-backed adapter anyway.

`warren-jni`'s `tunnel` Cargo feature is now **on by default**. The
`connectTunnel` JNI export takes the fd from Kotlin, constructs an
`AndroidTun` (which `fcntl`s the fd non-blocking), and pins it in the
`ACTIVE_TUNNEL` slot. The Quinn pump spawning + `ClientConfig` wiring is
the remaining work for D.4 step 2 (Section 6 below).

## 6. Open D.4 work items

Cross-repo `warren-core` portion (~Section 5~ **DONE** `d59fd15`):

- ~`warren-core`: add `PacketDevice::from_fd(fd: OwnedFd)` constructor +
  feature-gate `tun-rs` to `not(target_os = "android")`.~

Remaining `warren-app` D.4 work:

- ~`warren-jni`: enable `features = ["tunnel"]` by default.~ **DONE** (post
  `d59fd15` pin bump). `connectTunnel` now instantiates `AndroidTun`.
- `warren-jni`: write the real `connectTunnel` body - deserialize the JSON
  `WarrenTunnelConfig` blob, build `warren_tunnel::ClientConfig` +
  `ClientSession`, spawn the Quinn pump on the shared runtime against
  `AndroidTun`, and (when an entry hop is set) hand off to
  `warren_multihop::MultiHopClient`.
- ~`warren-app/android`: drop `WarrenDaemon.kt`~ **DONE** (`6076445631`).
- ~Drop `DaemonConfig` dataclass + Koin factory~ **DONE** (`b189bd73b1`).
- `WarrenVpnService.kt`: drop `managementService.start()` /
  `connectionProxy.*` calls (dead at runtime, no daemon), wire
  `WarrenQuinnAdapter` to invoke `WarrenJni.connectTunnel` once the
  VpnService Builder has handed back a TUN fd.
- Drop `lib/talpid/` module entirely once `WarrenVpnService` no longer
  extends `TalpidVpnService`.

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
