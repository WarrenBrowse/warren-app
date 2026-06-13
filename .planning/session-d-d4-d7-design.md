# Sessions D.4 -> D.7 design, Android Warren VPN

Single follow-up design doc covering the four heavy sub-phases that were
**not** delivered as code in the Session D wall-clock window:

- **D.4** VpnService Quinn (replaces WireGuard tunnel), 10-14 days
- **D.5** UI Compose wallet (BIP39 + Ed25519 + Keystore), 5-7 days
- **D.6** Multi-hop + DAITA + NAT-PMP UI parity, 5-7 days
- **D.7** Build signed APK + Play Store internal-test track, 3-5 days

Sessions D.1-D.3 landed mechanical rebrand + crate skeleton. The work
captured here is the **runtime + UX glue** that turns the rebrand into a
shippable Warren Android beta.

Each section enumerates: target architecture, file inventory, dependency
graph, build/test loop, and the explicit open items that need an Android
emulator (i.e. cannot be desk-checked).

---

## D.4, VpnService Quinn

### Goal

Replace the Mullvad Talpid tunnel layer (`android/lib/talpid/` +
`MullvadVpnService` -> `MullvadDaemon` JNI -> `mullvad-daemon` Rust state
machine -> WireGuard userspace) with a direct path:

```
[ Kotlin Activity ]
       v
[ WarrenVpnService : VpnService ]
       v  (composition)
[ WarrenQuinnAdapter ]
       v  (JNI)
[ WarrenJni.connectTunnel(tunFd, configJson) ]
       v
[ warren-tunnel Quinn pump on shared tokio runtime ]
       v  (optional)
[ warren-multihop entry hop -> Warren exit ]
```

### Architectural decisions

- **No Talpid carryover.** The Mullvad Talpid layer was built around the
  Linux daemon split (Rust daemon -> systemd-like supervisor); the
  Android `TalpidVpnService` is just a thin shim around that. Warren's
  state machine lives **in Kotlin** because: (a) the only async surface we
  need is reconnect-on-network-change, which has a first-class
  `ConnectivityManager.NetworkCallback` API; (b) keeping the daemon out of
  the Android process saves us the gRPC management interface entirely
  (Section 1 of D.3 design doc).

- **PacketDevice from fd.** The TUN file descriptor returned by
  `VpnService.Builder.establish()` is duped via Kotlin
  (`ParcelFileDescriptor.detachFd()`), passed as a `jint` to
  `WarrenJni.connectTunnel`, and wrapped on the Rust side by a new
  `warren_tunnel::PacketDevice::from_fd(OwnedFd)` constructor. **This is
  the cross-repo change that gates the whole D.4 PR**: it lives in
  `warren-core/crates/warren-tunnel/src/`, behind
  `#[cfg(target_os = "android")]`, gated by the `tunnel` feature in
  `warren-jni`.

- **JSON config bridge.** `WarrenTunnelConfig` is a Kotlin data class
  serialised via `kotlinx.serialization.json` and re-parsed on the Rust
  side via `serde_json`. Avoids the `jnix` `IntoJava`/`FromJava` ceremony
  for a config blob that mutates once per session.

- **Single tunnel slot.** Android's VpnService model is single-tunnel;
  `ACTIVE_TUNNEL: Mutex<Option<TunnelHandle>>` in `warren-jni/src/lib.rs`
  reflects that. Reconnect is **stop + start** with the same config blob.

### File inventory

To create in `warren-app`:

```
android/app/src/main/kotlin/com/warrenbrowse/vpn/app/service/
  WarrenQuinnAdapter.kt        # composition class owned by WarrenVpnService
  WarrenTunnelConfig.kt        # data class + serialisation
  WarrenTunnelState.kt         # sealed class (Disconnected, Connecting,
                               #   Connected, Reconnecting, Failed)
  reconnect/NetworkCallback.kt # ConnectivityManager.NetworkCallback impl

android/app/src/main/kotlin/com/warrenbrowse/vpn/jni/
  WarrenJni.kt                 # already shipped in D.3 (declarations only)
```

To rewrite in `warren-app`:

```
android/app/src/main/kotlin/com/warrenbrowse/vpn/app/service/
  WarrenVpnService.kt          # currently a Mullvad shim; rewrite to own
                               #   WarrenQuinnAdapter + handle lifecycle
                               #   callbacks (onStartCommand /
                               #   onRevoke / onDestroy)
```

To delete (or strip down):

```
android/lib/talpid/                          # entire Mullvad daemon connector
android/app/src/main/kotlin/com/warrenbrowse/vpn/app/service/
  WarrenDaemon.kt              # D.3 shim, no longer needed
```

Cross-repo change in `warren-core`:

```
crates/warren-tunnel/src/
  android.rs                   # new file, target_os = "android" only
    pub struct AndroidPacketDevice { fd: OwnedFd, ... }
    impl PacketDevice for AndroidPacketDevice { ... }
  real_tun.rs                  # cfg-gate tun-rs imports to not(android)
  lib.rs                       # mod android under cfg(target_os="android")
```

### Build + test loop

1. Add the `android.rs` `PacketDevice` in `warren-core`, push to a branch.
2. Update warren-app `warren-jni/Cargo.toml` to enable `features = ["tunnel"]`.
3. Run `cargo check -p warren-jni --target aarch64-linux-android` (host
   pre-flight, see D.3 design doc Section 4 for env vars).
4. Build the debug APK from Android Studio (or `./gradlew app:assembleOssProdDebug`).
   The Gradle `cargo {}` block invokes `cargo build --target <abi>
   --package warren-jni --release` per ABI and copies the resulting
   `libwarren_jni.so` into `build/rustJniLibs/android/<abi>/`.
5. Install on emulator (`adb install app-oss-prod-debug.apk`).
6. Smoke: open Warren, hit Connect, watch logcat
   (`adb logcat -s WarrenJni:V WarrenVpnService:V`).

### Open items needing an emulator

- DNS leak test (resolves through the tunnel exit, not the local DNS).
- IPv6 leak prevention (VpnService.Builder.addAddress + addRoute IPv6
  ranges).
- Network handover Wi-Fi -> cellular (toggle Wi-Fi in emulator settings,
  confirm tunnel reconnects within Backoff::HANDSHAKE 15 s).
- Lockdown mode (`setAlwaysOn` + `setLockdownEnabled = true`).
- Foreground service notification with type `systemExempted` (Android 14+).

---

## D.5, UI Compose wallet

### Goal

Replace the Mullvad account-number UX with Warren's Ed25519 BIP39 wallet:
generate or import a 12-word mnemonic, persist the derived signing key in
Android Keystore + `EncryptedSharedPreferences`, gate sensitive operations
behind `BiometricPrompt`.

### Architectural decisions

- **Mnemonic crosses the JNI once.** The BIP39 phrase is generated on the
  Rust side via `warren_identity::Mnemonic::generate_in(...)`, returned to
  Kotlin as a `String` for one screen (the "Back up your phrase" CTA).
  Kotlin immediately wraps it with AES-256-GCM via Android Keystore and
  hands the ciphertext to `EncryptedSharedPreferences`. The plaintext
  never lives in any persistent Kotlin field.

- **No clipboard CTA.** Per the brief (D.5.6), Warren omits the
  copy-mnemonic affordance to defeat clipboard-scraping malware. Display
  uses a `Box { ... }` with `if (revealed) word else "***"` toggled by a
  long-press or biometric reveal.

- **Two-factor optional.** A device-credential `BiometricPrompt`
  (Fingerprint / Face / PIN) gates: (a) "View mnemonic" Settings entry,
  (b) "Restore wallet" flow when overwriting an existing wallet.

- **No Google Drive backup.** Strictly device-local. Users back up via the
  printed mnemonic phrase.

### File inventory

To create:

```
android/lib/feature/wallet/api/                 # new feature module
  src/main/kotlin/com/warrenbrowse/vpn/feature/wallet/api/
    WalletState.kt
    WalletRepository.kt          # interface
android/lib/feature/wallet/impl/                # new feature module
  src/main/kotlin/com/warrenbrowse/vpn/feature/wallet/impl/
    WalletRepositoryImpl.kt      # AndroidKeystore + EncryptedSharedPreferences
    MnemonicInput.kt             # 12-word input composable
    MnemonicDisplay.kt           # blur+reveal display composable
    BiometricGate.kt             # BiometricPrompt wrapper

android/lib/feature/login/impl/src/main/kotlin/com/warrenbrowse/vpn/feature/login/impl/
  LoginScreen.kt                 # rewrite: drop AccountNumberInput, use
                                 #   MnemonicInput
  SignupWizard.kt                # 5-step (intro -> generate -> backup
                                 #   confirm -> create -> success)
```

To update in `settings.gradle.kts`:

```
include(
    ":lib:feature:wallet:impl",
    ":lib:feature:wallet:api",
)
```

i18n strings to add (`android/lib/ui/resource/src/main/res/values/strings.xml`
and `values-fr/strings.xml`):

```
<string name="wallet_create_title">Create your Warren wallet</string>
<string name="wallet_mnemonic_backup_warning">Write down these 12 words. They are the only way to recover your account.</string>
<string name="wallet_mnemonic_reveal">Tap to reveal</string>
<string name="wallet_import_title">Restore from mnemonic</string>
<string name="wallet_biometric_reason_view">Confirm to view your recovery phrase</string>
... (FR translations live alongside in values-fr)
```

### Build + test loop

- Espresso / Compose UI tests in
  `android/lib/feature/wallet/impl/src/androidTest/`:
  - `MnemonicInputTest` (12 fields, paste flow, validation).
  - `BiometricGateTest` (mocks `BiometricPrompt`).
- Manual emulator smoke:
  - Cold install -> Signup wizard -> generate mnemonic -> back up
    confirmation (Sentry-style "type word 3, word 7, word 11") -> wallet
    persisted across restart.
  - Wipe app data -> import same mnemonic -> derived pubkey matches.

---

## D.6, Multi-hop + DAITA + NAT-PMP UI parity

### Goal

Parity with the desktop M4.H.C surface + Session B onboarding for these
three Warren differentiators:

- **Multi-hop**: entry country picker + exit country picker, status banner.
- **DAITA**: toggle in Privacy settings, tooltip explaining traffic
  analysis defence.
- **M4.0 obfuscation**: indicator in the connection details panel.
- **NAT-PMP**: settings toggle + assigned port display + lifetime + UI in
  Settings (parallels desktop M4.H.F).
- **Multi-exit failover**: status banner with `failover_count` +
  `last_failover_age` (B.2 wire).
- **Country picker**: hydrated via `WarrenJni.listRelays() -> JSON ->
  com.warrenbrowse.vpn.feature.location.api.Relay`.

### File inventory

```
android/lib/feature/multihop/impl/src/main/kotlin/.../multihop/impl/
  MultiHopScreen.kt              # entry + exit picker
  MultiHopViewModel.kt           # state holder
android/lib/feature/daita/impl/src/main/kotlin/.../daita/impl/
  DaitaToggle.kt                 # already exists upstream, rebrand only
  DaitaTooltip.kt                # Warren-specific copy
android/lib/feature/home/impl/src/main/kotlin/.../home/impl/connect/
  ConnectionDetails.kt           # add M4.0 obfuscation indicator
  FailoverBanner.kt              # B.2 parity
android/lib/feature/vpnsettings/impl/src/main/kotlin/.../vpnsettings/impl/
  NatPmpSettings.kt              # toggle + port display
```

JSON shape from `WarrenJni.listRelays()` (already designed in warren-core):

```json
[
  {
    "exit_id": "exit-de-fra-1",
    "country_code": "DE",
    "city": "Frankfurt",
    "endpoint": "1.2.3.4:7777",
    "supports_daita": true,
    "supports_natpmp": true,
    "supports_multi_hop_entry": false,
    "load_pct": 42
  },
  ...
]
```

### Build + test loop

- Compose previews per screen (Compose Preview annotation).
- Espresso flow: enable multi-hop -> pick entry FR -> pick exit DE ->
  Connect -> verify banner shows multi-hop state -> toggle DAITA ->
  reconnect -> verify DAITA indicator -> enable NAT-PMP -> verify
  assigned port surfaces.

---

## D.7, Build signed APK + Play Store internal-test

### Goal

Produce a signed Release APK + .aab bundle, smoke 7-8 scenarios on an
emulator, and (if a production keystore is available) upload to the Play
Store internal-test track.

### Build path

```
# All flavors disabled except `playProdRelease` for the Play Store
# upload, `ossProdRelease` for the F-Droid-style direct APK download.
ANDROID_HOME=$HOME/Library/Android/sdk \
ANDROID_NDK_HOME=$HOME/Library/Android/sdk/ndk/29.0.13113456 \
./gradlew --no-daemon app:assembleOssProdRelease app:bundlePlayProdRelease
```

The resulting artefacts land in `android/app/build/outputs/`:
- `apk/ossProd/release/app-oss-prod-release.apk`
- `bundle/playProdRelease/app-play-prod-release.aab`

### Signing

- **Dev keystore (agent-generated, OK to use during D.7 dry-runs).**

  ```
  keytool -genkeypair \
    -alias warren-dev \
    -keystore android/warren-dev.jks \
    -keyalg RSA -keysize 4096 \
    -validity 10000 \
    -storepass <agent-generated> \
    -keypass <agent-generated>
  ```

  The `.jks` is gitignored (parity with desktop `.cer` files). Path:
  `android/warren-dev.jks` (referenced from `signingConfigs.release {}`
  block to be added in `app/build.gradle.kts`).

- **Production keystore (case 4 escalation, poka).** The `.jks` for the
  Play Store upload must be generated by poka on his trusted machine,
  stored in 1Password, and shared as base64 via `WARREN_ANDROID_KEYSTORE_B64`
  env var to the GitHub Actions release workflow (parity with
  `WARREN_CSC_KEYSTORE_B64` for desktop M4.H.D).

### Play Store metadata

Required for an internal-test upload (Google Play Console):

```
android/app/src/main/play/
  listings/en-US/
    title.txt                   # "Warren VPN" (max 30 chars)
    short-description.txt       # max 80 chars
    full-description.txt        # max 4000 chars
  listings/fr-FR/
    (same structure, French translations)
  release-notes/en-US/default.txt
  release-notes/fr-FR/default.txt
```

URLs (already in `strings_non_translatable.xml` from D.1):
- Privacy policy: `https://warrenbrowse.com/privacy`
- Support: `support@warrenbrowse.com`
- Website: `https://warrenbrowse.com/download`

### Emulator smoke checklist

1. Cold install -> onboarding wizard -> generate mnemonic -> wallet
   persisted.
2. Connect to Warren exit-1 prod -> tunnel status reaches `Connected`.
3. DNS leak test (browse to `https://dnsleaktest.com` -> verify exit-1 IP
   resolves DNS).
4. Toggle multi-hop -> pick entry / exit -> reconnect.
5. Toggle DAITA -> reconnect -> verify DAITA indicator in details panel.
6. Toggle NAT-PMP -> verify assigned port renders.
7. Disconnect -> Reconnect.
8. Network change handover (toggle Wi-Fi in emulator AVD settings ->
   verify tunnel reconnects within 30 s).

### Open items needing poka

- Production `.jks` keystore + upload to 1Password.
- `PLAY_CREDENTIALS_PATH` Google Play Developer API key in 1Password +
  `WARREN_PLAY_CREDENTIALS_B64` GitHub secret.
- Google Play Console account + Warren VPN app entry + internal-test
  track configuration.

---

## Sequencing dependencies

```
D.4 (warren-core PacketDevice + warren-app Adapter + VpnService rewrite)
 |
 +-> requires D.3 ✓
 |
 v
D.5 (UI Compose wallet, indep of D.4)
 |
 +-> requires D.3 ✓ (WarrenJni signing/mnemonic stubs lit up)
 |
 v
D.6 (multi-hop / DAITA / NAT-PMP UI, depends on D.4 connect + D.5 wallet)
 |
 v
D.7 (build + smoke, depends on D.4-D.6 + production keystore from poka)
```

D.5 can run in parallel with D.4 (different file trees, no overlap). D.6
must wait on D.4 to have a working tunnel to drive its toggles.

The conservative wall-clock for the remaining D.4 -> D.7 stack is **3-5
weeks of focused work**, assuming poka unblocks the production keystore
in parallel.
