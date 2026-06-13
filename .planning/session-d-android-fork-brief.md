# Session D, Android fork Mullvad → Warren VPN mobile

> Brief d'agent autonome cross-repo warren-core + warren-app.
> Doctrine §0.0 INVIOLABLE destructive git + §0.5 full autonomy.
> Grosse session séquentielle : l'agent enchaîne D.1 → D.7 sans escalade.

**Effort estimé** : wall-clock 1-2 mois (7 sous-phases).
**Coût Hetzner** : 0 EUR (Android emulator + Play Store internal testing suffisent).
**Pré-conditions** :
- warren-app `main` HEAD `2c49588b22+` (post-Session B)
- warren-core `main` HEAD `ba819cf+` (post-Session B)
- Android SDK + NDK + Gradle 8+ requis (peut tourner cross-platform Linux/Mac)

**Objectif** : livrer Warren VPN Android bêta Google Play internal-test-ready à partir du fork Mullvad Android upstream présent dans `warren-app/android/` et `warren-app/mullvad-jni/`.

Sous-phases (séquentielles autonomes) :

1. **D.1, Rebrand Gradle + applicationId** (~2-3j)
2. **D.2, Rebrand Kotlin namespace + package** (~3-5j)
3. **D.3, `mullvad-jni` → `warren-jni` crate + wire warren-core** (~5-7j)
4. **D.4, VpnService Quinn (replace WireGuard tunnel)** (~10-14j)
5. **D.5, UI Compose adapt wallet Ed25519 mnemonic auth** (~5-7j)
6. **D.6, Multi-hop + DAITA + NAT-PMP UI parity** (~5-7j)
7. **D.7, Build APK signed + smoke Android emulator** (~3-5j)

---

## 0.0 INVIOLABLE, pas de commande git destructive

Cf. doctrine standard. Préserver fichiers modified ou untracked. Submodule `android/rust-android-gradle-plugin` peut nécessiter init (`git submodule update --init android/rust-android-gradle-plugin`), c'est OK (additif, pas destructif).

Violation = scope error CRITIQUE. Incident M4.H.F.

---

## 0.5 MANDAT D'AUTONOMIE

Cf. memory `feedback_agent_full_autonomy_no_timid_rollback`. Plein mandat.

Escalade `AskUserQuestion` SEULEMENT si :
1. Secret leak (.jks keystore, Google Play API key, mnemonic)
2. Coût > 0.30 EUR (n/a)
3. Breaking change /v1 wire format
4. Signing key prod (.jks Android upload key)
5. **Spécifique session D** : si tu détectes que mullvad-daemon Rust path (consumed par mullvad-jni) a une surface trop grosse à rebrand intégralement, escalade pour stratégie (rebrand complet vs Warren-specific subset)

Décisions tactiques agent autorisées :
- Min SDK : 26 (Android 8.0) ou 28 (9.0), recommandation 26, marché Android étendu
- Target SDK : latest stable (34/35)
- Build system : Gradle (déjà installé, pas changer pour Bazel)
- App flavors : drop `devmole`, `stagemole`, `leakcanary` Mullvad-specific, garder `dev` + `release`
- Compose version : suivre BOM Mullvad upstream
- Keystore : escalade poka pour .jks production, dev keystore agent OK

---

## 1. Setup initial

```bash
cd /Users/poka/dev/warrenBros/warren-app
git status                                  # clean main 2c49588b22+
cd android
ls -d app lib                                # confirme Gradle modules
./gradlew --version                          # confirme Gradle wrapper
git submodule update --init android/rust-android-gradle-plugin
```

Build pré-flight :
```bash
cd /Users/poka/dev/warrenBros/warren-app/android
./gradlew app:tasks                          # liste targets app
```

Si Android SDK/NDK pas configuré : escalade pour `ANDROID_HOME` + `NDK_HOME` env vars.

---

## 2. Optimisations agent

- Read sources cross-repo + Gradle + Kotlin en PARALLÈLE
- Tests TDD JUnit groupés en fin de sous-phase
- Push warren-core + warren-app au fil de l'eau
- Gradle `--parallel` flag pour builds multi-module
- Rebrand sed opportuniste mais lire avant : `net.mullvad.mullvadvpn` est partout (package Kotlin, namespace, applicationId, providerAuthority, etc.)

---

## D.1, Rebrand Gradle + applicationId (~2-3j)

### Scope D.1

1. **D.1.1** Renommer projet Gradle name dans `settings.gradle.kts`
2. **D.1.2** `app/build.gradle.kts` :
   - `namespace = "net.mullvad.mullvadvpn"` → `namespace = "com.warrenbrowse.vpn"`
   - `applicationId = "net.mullvad.mullvadvpn"` → `applicationId = "com.warrenbrowse.vpn"`
3. **D.1.3** Drop flavors `devmole` + `stagemole` + `leakcanary` (Mullvad-specific)
4. **D.1.4** App name : `strings.xml` `<string name="app_name">Mullvad VPN</string>` → `Warren VPN`
5. **D.1.5** App icon : remplacer `app/src/main/res/mipmap-*/ic_launcher*.png` par Warren logo (W jaune navy, cohérent iOS C.1.5 + desktop)
6. **D.1.6** Splash screen : `res/values/themes.xml` + `splash_screen` drawable adapter
7. **D.1.7** Version : `appVersion.code` + `appVersion.name` config dans `config/`
8. **D.1.8** `./gradlew app:assembleDebug` PASS

### Critères GO D.1

- Gradle build PASS
- APK généré avec applicationId `com.warrenbrowse.vpn`
- App name "Warren VPN" dans launcher (test emulator)
- Icon Warren

### Décisions tactiques D.1

- applicationId `com.warrenbrowse.vpn` (parité desktop M4.H.D + iOS)
- Min SDK 26 (Android 8.0+)
- Target SDK 34 (déjà Mullvad)

---

## D.2, Rebrand Kotlin namespace + package (~3-5j)

### Scope D.2

1. **D.2.1** Renommer `app/src/main/kotlin/net/mullvad/mullvadvpn/` → `app/src/main/kotlin/com/warrenbrowse/vpn/`
2. **D.2.2** Idem pour `lib/`, `test/`, `androidTest/` sous-dossiers
3. **D.2.3** `package net.mullvad.mullvadvpn.*` → `package com.warrenbrowse.vpn.*` dans tous les .kt files (rg + sed)
4. **D.2.4** Imports `net.mullvad.mullvadvpn.*` → `com.warrenbrowse.vpn.*`
5. **D.2.5** ContentProvider authority : `net.mullvad.mullvadvpn.<x>` → `com.warrenbrowse.vpn.<x>`
6. **D.2.6** Drop Kotlin classes Mullvad-specific (PostQuantum WG, Shadowsocks bridge protocols)
7. **D.2.7** Renommer classes `Mullvad*` → `Warren*` (ex: `MullvadVpnService.kt` → `WarrenVpnService.kt`, `MullvadApplication.kt` → `WarrenApplication.kt`)
8. **D.2.8** AndroidManifest.xml : refs `.mullvad.` → `.warrenbrowse.`
9. **D.2.9** Tests : `./gradlew app:testDebugUnitTest` PASS

### Critères GO D.2

- Tous package + namespace cohérents `com.warrenbrowse.vpn`
- Build Debug PASS
- Tests unit PASS (modulo tests Mullvad-specific droppés)

### Décisions tactiques D.2

- Strategy sed : per-package via `find ... -name '*.kt' -exec sed ...`, valider per-package avant batch
- Garder `Operations` package (générique) si présent

---

## D.3, `mullvad-jni` → `warren-jni` crate + wire warren-core (~5-7j)

### Scope D.3

1. **D.3.1** Renommer crate `mullvad-jni` → `warren-jni` (Cargo.toml + workspace path)
2. **D.3.2** Réécrire deps : `mullvad-daemon` → DROP (trop gros, Warren mobile = client only pas daemon full), `mullvad-api` → `warren-api-client`
3. **D.3.3** Wire `warren-tunnel` + `warren-client` + `warren-identity` + `warren-multihop` + `warren-natpmp-client` (path-dep warren-core)
4. **D.3.4** Réécrire `lib.rs` JNI exports :
   - `Java_com_warrenbrowse_vpn_jni_WarrenJni_initLogger`
   - `Java_com_warrenbrowse_vpn_jni_WarrenJni_connectTunnel` (warren-tunnel params)
   - `Java_com_warrenbrowse_vpn_jni_WarrenJni_disconnectTunnel`
   - `Java_com_warrenbrowse_vpn_jni_WarrenJni_generateMnemonic` (warren-identity BIP39)
   - `Java_com_warrenbrowse_vpn_jni_WarrenJni_importMnemonic`
   - `Java_com_warrenbrowse_vpn_jni_WarrenJni_signRequest` (Ed25519 wallet)
   - `Java_com_warrenbrowse_vpn_jni_WarrenJni_listRelays` (warren-relay-selector)
   - Etc.
5. **D.3.5** `jnix` macros (déjà dep mullvad-jni) pour structures Rust ↔ Java
6. **D.3.6** cbindgen non requis Android (vs iOS), JNI via `jnix` derive
7. **D.3.7** Tests Rust JNI : tests in-process avec mock JVM (cf. patterns mullvad-jni existants)

### Critères GO D.3

- Crate `warren-jni` compile pour `target_os = "android"` (cargo build --target aarch64-linux-android + x86_64 emulator + armv7-linux-androideabi)
- `.so` généré dans `target/aarch64-linux-android/release/libwarren_jni.so`
- Tests Rust PASS

### Décisions tactiques D.3

- DROP `mullvad-daemon` dep : warren-jni = light client (pas daemon full state machine). Tunnel state machine Warren mobile vit Kotlin-side via VpnService + warren-jni FFI minimal.
- cdylib pour Android (vs staticlib iOS), ne pas changer
- `jnix` reste, c'est juste un wrapper macro lib

---

## D.4, VpnService Quinn (replace WireGuard tunnel) (~10-14j)

### Scope D.4

C'est la sous-phase la plus complexe. Android VpnService doit :
- Accepter VPN config depuis Activity
- Établir tunnel via warren-jni + warren-tunnel Quinn (vs WireGuard userspace Mullvad)
- Router IP packets entre TUN virtual (ParcelFileDescriptor) et warren-tunnel
- Reconnect on network change (handover Wi-Fi ↔ cellular ↔ ethernet USB)
- Killswitch via VpnService block mode

1. **D.4.1** Étudier `WarrenVpnService.kt` (post-D.2 rename) WireGuard impl
2. **D.4.2** Remplacer WG userspace adapter par `WarrenQuinnAdapter.kt` :
   - `init(vpnService: VpnService)` 
   - `startTunnel(config: WarrenTunnelConfig)` suspend
   - `stopTunnel()` suspend
   - `handleNetworkChange()` ConnectivityManager.NetworkCallback
   - Internal : spawn Quinn via warren-jni FFI, route packets via tun fd
3. **D.4.3** Config translation : `WarrenTunnelConfig` Kotlin data class → JSON serialize → warren-jni FFI → `WarrenTunnelParameters` Rust struct. Inclut : exit pubkey, exit IP:port, wallet signing key (envoyé via Keystore secure pipe), multi-hop relay config (optional), DAITA spec (optional), NAT-PMP enabled (optional), bypass_cidrs (optional)
4. **D.4.4** Killswitch Android : VpnService `setAlwaysOn` + `setLockdownEnabled` configurable user. UI Settings expose toggle.
5. **D.4.5** Reconnect : ConnectivityManager.NetworkCallback détecte handover, trigger reconnect via Backoff::HANDSHAKE 15s
6. **D.4.6** AndroidManifest.xml : `<service android:name=".vpn.WarrenVpnService" android:permission="android.permission.BIND_VPN_SERVICE" ...>`
7. **D.4.7** Tests : tests Kotlin `WarrenVpnServiceTest` + tests Rust FFI warren-jni

### Critères GO D.4

- VpnService connect Warren tunnel OK Android emulator
- Disconnect OK
- Network change handover sans drop tunnel
- Killswitch active (lockdown mode)
- DNS leak test PASS
- IPv6 leak prevention OK

### Décisions tactiques D.4

- DROP `wireguard-go` integration : Warren utilise Quinn pur Rust
- DROP `tunnel-obfuscation` dep : obfuscation M4.0 intégrée warren-tunnel
- KEEP ParcelFileDescriptor TUN router pattern : compat Android standard

---

## D.5, UI Compose adapt wallet Ed25519 mnemonic auth (~5-7j)

### Scope D.5

1. **D.5.1** Login screen Compose : remplacer `AccountNumberInput` par `MnemonicInput` (12-word BIP39)
2. **D.5.2** Signup wizard mobile Compose : 5-step parité iOS C.5.2 + desktop session B
3. **D.5.3** Wallet storage : Android Keystore + EncryptedSharedPreferences pour mnemonic + signing key
4. **D.5.4** Restore flow : Settings → "Restore wallet" → MnemonicInput
5. **D.5.5** Backup CTA : "View mnemonic" Settings, BiometricPrompt (Face/Fingerprint Auth) gating
6. **D.5.6** Mnemonic display security : pas de CTA "Copier" (anti-malware-clipboard), blur+reveal
7. **D.5.7** i18n FR + EN minimum (`strings.xml` + `strings-fr.xml`)

### Critères GO D.5

- Login + Signup Compose fonctionnels
- Wallet Keystore storage encrypted
- Restore + Backup flows
- BiometricPrompt gating
- i18n FR+EN
- Tests UI `androidTest/` PASS

### Décisions tactiques D.5

- Android Keystore : KeyStore.getInstance("AndroidKeyStore"), KeyGenParameterSpec.Builder, AES-256-GCM wrap mnemonic
- BiometricPrompt : androidx.biometric, fallback device credential
- No backup to Google Drive : strictement device-local

---

## D.6, Multi-hop + DAITA + NAT-PMP UI parity (~5-7j)

### Scope D.6

Parité avec desktop M4.H.C + session B + iOS C.6 :

1. **D.6.1** Multi-hop view Compose : entry + exit country pickers
2. **D.6.2** DAITA toggle Compose : settings section privacy, tooltip
3. **D.6.3** Obfuscation M4.0 indicator : connection-details
4. **D.6.4** NAT-PMP settings : enabled toggle + port forwarded display + lifetime
5. **D.6.5** Multi-exit failover : status banner
6. **D.6.6** Country picker : warren-relay-selector via warren-jni FFI
7. **D.6.7** Settings → Privacy preferences

### Critères GO D.6

- Multi-hop opérationnel Android (entry + exit pickers)
- DAITA toggle wire + applique
- Obfuscation indicator visible
- NAT-PMP UI fonctionnelle
- Failover banner

---

## D.7, Build APK signed + smoke Android emulator (~3-5j)

### Scope D.7

1. **D.7.1** Build Release APK + Bundle (.aab pour Play Store) :
   `./gradlew app:assembleRelease` + `./gradlew app:bundleRelease`
2. **D.7.2** Signing : agent peut générer dev keystore. Production keystore = escalation poka (case 4 signing key prod)
3. **D.7.3** Smoke Android emulator :
   - Onboarding generate wallet
   - Connect Warren exit-1 prod
   - DNS leak test
   - Multi-hop toggle + reconnect
   - DAITA toggle apply
   - NAT-PMP qBittorrent emulator (si feasible)
   - Disconnect + reconnect
   - Network change handover (Wi-Fi → cellular emulation)
4. **D.7.4** Bundle Play Store metadata : title, short description, full description (FR+EN), screenshots, privacy policy URL (warrenbrowse.com/privacy), content rating
5. **D.7.5** Google Play internal-test track upload : `gradle publishReleaseBundle` (Google Play Developer API), skip si .jks pending poka

### Critères GO D.7

- Build Release PASS
- APK signé dev keystore + .aab généré
- 7-8 smoke tests emulator PASS
- Play Store metadata complète (placeholder business OK)
- Play Store internal-test upload skip si keystore pending poka

### Décisions tactiques D.7

- Dev keystore agent : `keytool -genkeypair -alias warren-dev -keystore warren-dev.jks -keyalg RSA -keysize 4096 -validity 10000`. PAS commit la .jks dans le repo. Documenter path.
- Prod keystore : escalation poka (case 4)
- Internal-test vs Closed beta : internal-test (up to 100 testeurs sans review)

---

## 3. Sources cross-repo à lire (PARALLÈLE)

### warren-app Android surface
- `android/settings.gradle.kts` + `android/build.gradle.kts`
- `android/app/build.gradle.kts` (applicationId + namespace)
- `android/lib/build.gradle.kts` (modules Kotlin libs)
- `android/app/src/main/AndroidManifest.xml`
- `android/app/src/main/kotlin/net/mullvad/mullvadvpn/app/service/MullvadVpnService.kt`
- `android/app/src/main/res/values/strings.xml`
- `mullvad-jni/Cargo.toml` + `mullvad-jni/src/lib.rs`

### warren-core
- `crates/warren-tunnel/src/lib.rs`
- `crates/warren-api-client/src/lib.rs`
- `crates/warren-identity/src/lib.rs` (BIP39 + Ed25519)
- `crates/warren-multihop/src/lib.rs`
- `crates/warren-natpmp-client/src/lib.rs`
- `crates/warren-relay-selector/src/lib.rs`

### Référence Mullvad
- `android/app/src/main/kotlin/.../MullvadApplication.kt` (Application class)
- `android/lib/talpid/` (Mullvad daemon connector pattern)

---

## 4. Plan d'exécution (séquentiel, autonome)

```
D.1 Rebrand Gradle + applicationId (2-3j)
D.2 Rebrand Kotlin namespace (3-5j)
D.3 warren-jni crate + wire (5-7j)
D.4 VpnService Quinn (10-14j) ← phase la plus complexe
D.5 UI Compose wallet (5-7j)
D.6 UI multi-hop/DAITA/NAT-PMP parity (5-7j)
D.7 Build APK + smoke emulator (3-5j)
D.8 Rapport final (1h)
  └── .planning/session-d-report.md
```

Total ~1-1.5 mois wall-clock. Push warren-core (si modif) + warren-app au fil de l'eau.

---

## 5. Critères GO ULTIMATE session D

- ✅ D.1-D.7 critères GO PASS
- ✅ `./gradlew app:assembleDebug` + `./gradlew app:assembleRelease` PASS
- ✅ `cargo test --workspace` warren-core + warren-app PASS (pas de régression desktop)
- ✅ `cargo clippy --workspace --all-targets -- -D warnings` PASS
- ✅ `cargo build --target aarch64-linux-android -p warren-jni` PASS
- ✅ `cargo build --target armv7-linux-androideabi -p warren-jni` PASS
- ✅ `cargo build --target x86_64-linux-android -p warren-jni` PASS (emulator)
- ✅ Pas de régression Linux/Mac/Win desktop
- ✅ Rapport `.planning/session-d-report.md` rédigé

Verdict GO PARTIEL acceptable si :
- Play Store internal-test upload skipped (keystore pending poka)
- D.7 smoke emulator PASS suffit pour GO PARTIEL
- Device physical Android test skipped

---

## 6. Doctrine

- §0.0 INVIOLABLE git
- §0.5 autonomy
- English-only code comments (Kotlin `//`, Rust `//`)
- Pas em-dash
- Pas secrets in commits (.jks, Play Console API key, mnemonic)
- 5 concurrents comparison standard
- Pas Cure53 mention

---

## 7. Memory updates attendus

- `warren_session_d_delivered.md`, verdict + caveats par sous-phase
- Update `MEMORY.md` index
- Memory dédié si feature mobile non-triviale (ex: `warren_android_vpnservice_adapter.md`)

---

## 8. Commencer maintenant

Lis le brief, sources §3 en parallèle, attaque D.1.1. Plein mandat §0.5. Push au fil de l'eau. Android = ~30% audience VPN payant, important pour la base utilisateurs.

Bonne route.
