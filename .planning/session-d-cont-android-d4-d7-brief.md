# Session D-continuation, Android D.4-D.7 + fix tun_rs Android backend

> Brief d'agent autonome warren-app + warren-core.
> Doctrine §0.0 INVIOLABLE destructive git + §0.5 full autonomy + §0.6 WORKTREE SÉPARÉ OBLIGATOIRE.
> Continuation Session D partielle (D.1 + D.2 + D.3 skeleton livrés 2026-05-21).

**Effort estimé** : wall-clock 3-5 semaines (4 sous-phases restantes + tun_rs Android backend fix).
**Coût Hetzner** : 0 EUR (Android emulator + Play Store internal-test suffisent).
**Pré-conditions** :
- warren-app `main` HEAD post-Session-G (DAITA pump fix livré, mandatory pour DAITA UI D.6)
- warren-core `main` HEAD post-Session-G
- Session D report : `.planning/session-d-report.md` (si présent) + memory `warren_session_d_delivered`
- Android SDK + NDK + Gradle 8+ installés

**Objectif** : compléter le fork Android Mullvad → Warren VPN mobile, livrer Play Store internal-test-ready bêta.

Sous-phases restantes (séquentielles autonomes) :

1. **D-cont.1, Setup worktree warren-app dédié Android** (~30 min)
2. **D-cont.2, Fix tun_rs Android backend OR custom PacketDevice** (~3-5j, blocker D.4)
3. **D.4, VpnService Quinn full rewrite** (~10-14j)
4. **D.5, UI Compose wallet Ed25519 BIP39** (~5-7j)
5. **D.6, Multi-hop + DAITA + NAT-PMP UI parity** (~5-7j, dépend Session G)
6. **D.7, Build APK signed + smoke Android emulator + Play Store internal-test** (~3-5j)

---

## 0.0 INVIOLABLE, pas de commande git destructive

Cf. doctrine standard. Submodule `android/rust-android-gradle-plugin` peut nécessiter init (additif, non destructif).

Violation = scope error CRITIQUE.

---

## 0.5 MANDAT D'AUTONOMIE

Plein mandat. Escalade SEULEMENT si secret leak, coût > 0.30 EUR, breaking /v1, signing key prod, OU **spécifique session D-cont** : si tun_rs Android backend fix demande refactor warren-tunnel majeur (> 5j seul), escalader pour validation archi.

Décisions tactiques agent autorisées :
- tun_rs Android backend : contribute upstream tun_rs (PR à `tun-rs-rs/tun-rs`) OR fork local OR custom Warren impl via VpnService.Builder().establish() → ParcelFileDescriptor → OwnedFd direct (bypass crate)
- Min SDK 26 (Android 8.0+)
- Target SDK 34 ou 35
- Keystore : dev keystore auto-générée par agent OK, prod escalation case 4

---

## 0.6 WORKTREE SÉPARÉ OBLIGATOIRE (CRITIQUE)

Incident 2026-05-21 Session C+D parallèles même worktree. Cette continuation ne doit JAMAIS partager worktree avec C-cont ou autres sessions.

```bash
cd /Users/poka/dev/warrenBros/warren-app
git fetch origin
git worktree add ../warren-app-android-d-cont main
cd ../warren-app-android-d-cont
```

Cleanup en fin :
```bash
cd /Users/poka/dev/warrenBros/warren-app
git worktree remove ../warren-app-android-d-cont
```

NE PAS travailler dans `/Users/poka/dev/warrenBros/warren-app` directement.

---

## 1. Setup initial

```bash
cd /Users/poka/dev/warrenBros/warren-app
git fetch origin
git worktree add ../warren-app-android-d-cont main
cd ../warren-app-android-d-cont

git log --oneline -10                       # confirme D.1+D.2+D.3 skeleton + Session G présent
ls android/app/build.gradle.kts
grep 'applicationId' android/app/build.gradle.kts  # confirme com.warrenbrowse.vpn
ls warren-jni
git submodule update --init android/rust-android-gradle-plugin
```

Lire artifacts Session D :
```bash
cat /Users/poka/.claude/projects/-Users-poka-dev-warrenBros-warren-app/memory/warren_session_d_delivered.md
```

---

## 2. Optimisations agent

- Gradle `--parallel` flag pour builds multi-module
- Cross-compile cargo --target {aarch64,armv7,x86_64}-linux-android en parallèle
- Tests JUnit groupés en fin de sous-phase

---

## D-cont.1, Setup worktree Android (~30 min)

Cf. §0.6.

### Critères GO D-cont.1

- Worktree opérationnel
- HEAD warren-app post-Session-G confirmé
- Artifacts Session D + applicationId rebrand confirmés

---

## D-cont.2, Fix tun_rs Android backend OR custom PacketDevice (~3-5j)

### Contexte

Session D blocker connu : warren-tunnel `PacketDevice::from_fd(OwnedFd)` cross-repo (`tun_rs 2.8` pas de backend Android). Feature `tunnel` warren-core off-by-default jusqu'au fix.

Sans ce fix, D.4 VpnService Quinn ne peut pas alimenter le tunnel warren-core. Workaround impératif avant D.4.

### Scope D-cont.2

3 stratégies possibles, à arbitrer §0.5 :

**Stratégie A, Contribute upstream tun_rs** :
- Implémenter `tun_rs::AsyncDevice::from_fd_android` upstream
- PR vers `tun-rs-rs/tun-rs`
- Wait merge + release tun_rs 2.9
- Update warren-core dep
- **Risque** : timing upstream merge incontrôlable (~semaines)

**Stratégie B, Fork local tun_rs** :
- Fork `tun_rs` dans `vendor/tun-rs-android/` ou submodule
- Implémenter backend Android : `VpnService.Builder().establish()` → ParcelFileDescriptor → fd → OwnedFd
- Wire Quinn UDP socket sur ce fd
- warren-core dep vers fork local
- **Risque** : maintenance burden fork

**Stratégie C, Custom Warren impl, bypass tun_rs** :
- warren-tunnel ajoute `#[cfg(target_os = "android")]` PacketDevice impl direct
- Pas de `tun_rs` Android, juste tokio I/O sur `OwnedFd` du ParcelFileDescriptor
- Reuse pattern Mullvad Android (déjà fait pour WireGuard upstream, à porter pour Quinn UDP)
- **Avantage** : pas de dep externe à maintenir, alignement Mullvad

Recommandation : **Stratégie C** (custom impl, alignement Mullvad WG pattern). Effort estimé 2-3j.

1. **D-cont.2.1** Étudier `talpid-tunnel` Mullvad pour pattern Android fd consumer (WireGuard upstream)
2. **D-cont.2.2** Implémenter `warren_tunnel::packet_device::android_fd` :
   - `pub struct AndroidPacketDevice { fd: AsyncFd<OwnedFd>, ... }`
   - `impl PacketDevice for AndroidPacketDevice { ... }` (read/write trait)
3. **D-cont.2.3** Activate `tunnel` feature `#[cfg(target_os = "android")]` warren-tunnel Cargo.toml
4. **D-cont.2.4** `warren-jni` expose `fn connect_with_fd(raw_fd: c_int, ...)` via JNI
5. **D-cont.2.5** Tests : Android emulator end-to-end fd consumer + Quinn tunnel

### Critères GO D-cont.2

- warren-tunnel Android backend opérationnel
- `cargo check --target aarch64-linux-android -p warren-tunnel --features tunnel` PASS
- warren-jni `connect_with_fd` câblé
- Tests Android emulator basic OK

---

## D.4, VpnService Quinn full rewrite (~10-14j)

Phase la plus complexe. Cf. brief original Session D §D.4 pour scope détaillé. Résumé :

1. `WarrenVpnService.kt` (post-D.2 rename) full rewrite : drop WG userspace, wire `WarrenQuinnAdapter`
2. `WarrenQuinnAdapter.kt` Kotlin : init/startTunnel/stopTunnel/handleNetworkChange + warren-jni FFI
3. Config translation `WarrenTunnelConfig` Kotlin → JSON → warren-jni → Rust struct
4. Killswitch via `setAlwaysOn` + `setLockdownEnabled`
5. Reconnect Backoff::HANDSHAKE 15s + ConnectivityManager.NetworkCallback
6. AndroidManifest.xml : permissions VpnService
7. Tests Kotlin + Rust FFI

### Critères GO D.4

- VpnService connect Warren tunnel Android emulator OK
- Disconnect OK
- Network change handover sans drop
- Killswitch lockdown mode active
- DNS leak test PASS
- IPv6 leak prevention OK

---

## D.5, UI Compose wallet Ed25519 BIP39 (~5-7j)

Cf. brief original Session D §D.5. Résumé :

1. Login Compose : `MnemonicInput` (12-word BIP39)
2. Signup wizard mobile 5-step (parité iOS C.5 + desktop session B)
3. Wallet storage : Android Keystore + EncryptedSharedPreferences
4. Restore + Backup flows
5. BiometricPrompt gating
6. Mnemonic blur+reveal sans clipboard CTA
7. i18n FR + EN

### Critères GO D.5

- Login + Signup Compose fonctionnels
- Keystore storage AES-256-GCM
- BiometricPrompt gating
- i18n FR+EN
- Tests UI `androidTest/` PASS

---

## D.6, Multi-hop + DAITA + NAT-PMP UI parity (~5-7j, dépend Session G)

⚠️ **DÉPENDANCE STRICTE** : Session G livré GO ULTIMATE avant D.6 DAITA UI. Sinon DAITA Android = feature non-shippable.

Cf. brief original Session D §D.6. Résumé :

1. Multi-hop Compose : entry + exit pickers
2. DAITA toggle Compose (post-Session G validé)
3. Obfuscation M4.0 indicator
4. NAT-PMP settings
5. Multi-exit failover banner
6. Country picker via warren-relay-selector FFI

### Critères GO D.6

- Multi-hop opérationnel Android
- DAITA toggle applique (post-Session G)
- Obfuscation indicator visible
- NAT-PMP UI fonctionnelle
- Failover banner

### Décisions tactiques D.6

- Si Session G NO-GO : DAITA UI Android feature-flagged OFF + hidden Settings
- Multi-hop + NAT-PMP indépendants Session G, peuvent ship

---

## D.7, Build APK signed + smoke Android emulator + Play Store internal-test (~3-5j)

Cf. brief original Session D §D.7. Résumé :

1. Build Release APK + Bundle (.aab)
2. Dev keystore agent OK, prod keystore = escalation case 4
3. Smoke Android emulator (7-8 tests + network handover)
4. Play Store metadata FR+EN
5. Internal-test upload skip si .jks prod pending poka

### Critères GO D.7

- Build Release + .aab PASS
- APK signé dev keystore
- 7-8 smoke tests emulator PASS
- Play Store metadata complète
- Internal-test upload skip OK si keystore pending poka

---

## 3. Sources cross-repo à lire (PARALLÈLE)

### warren-app Android surface (post-Session D skeleton)
- `android/settings.gradle.kts` + `android/build.gradle.kts`
- `android/app/build.gradle.kts` (applicationId com.warrenbrowse.vpn)
- `android/app/src/main/kotlin/com/warrenbrowse/vpn/` (1311 files renamed Session D.2)
- `android/app/src/main/AndroidManifest.xml`
- `warren-jni/Cargo.toml` + `warren-jni/src/lib.rs` (post-Session D.3 skeleton)
- `android/lib/talpid/` (Mullvad daemon connector pattern Android)

### warren-core path-deps
- `crates/warren-tunnel/src/packet_device.rs` (target D-cont.2 Android backend)
- `crates/warren-tunnel/src/lib.rs`
- `crates/warren-client/src/lib.rs`
- `crates/warren-identity/src/lib.rs`

### Référence
- `mullvad-jni/` legacy upstream patterns
- talpid-tunnel Mullvad Android WG pattern

---

## 4. Plan d'exécution (séquentiel)

```
D-cont.1 Worktree setup (30 min)
D-cont.2 tun_rs Android backend fix (3-5j) ← unblock D.4
D.4 VpnService Quinn full rewrite (10-14j) ← le plus complexe
D.5 UI Compose wallet (5-7j)
D.6 UI multi-hop/DAITA/NAT-PMP (5-7j, post-Session-G)
D.7 APK + Play Store smoke (3-5j)
D.8 Rapport + memory + cleanup
```

Total ~3-5 semaines wall-clock.

---

## 5. Critères GO ULTIMATE session D-cont

- ✅ D-cont.2 + D.4 + D.5 + D.6 + D.7 critères GO PASS
- ✅ `./gradlew app:assembleDebug` + `./gradlew app:assembleRelease` PASS
- ✅ `cargo build --target aarch64-linux-android -p warren-jni` PASS
- ✅ `cargo build --target armv7-linux-androideabi -p warren-jni` PASS
- ✅ `cargo build --target x86_64-linux-android -p warren-jni` PASS
- ✅ warren-tunnel `tunnel` feature Android activé
- ✅ `cargo test --workspace` warren-core + warren-app PASS (pas régression desktop)
- ✅ Worktree cleaned

Verdict GO PARTIEL acceptable si :
- Play Store internal-test upload skipped (.jks prod pending poka)
- Device physical Android smoke skipped (emulator suffit)
- D.6 DAITA UI feature-flagged OFF si Session G pas GO ULTIMATE

---

## 6. Doctrine

- §0.0 INVIOLABLE git
- §0.5 autonomy
- §0.6 worktree séparé obligatoire (CRITIQUE)
- English-only code comments (Kotlin + Rust)
- Pas em-dash
- Pas secrets in commits

---

## 7. Memory updates attendus

- `warren_session_d_cont_delivered.md`
- Update MEMORY.md

---

## 8. Commencer maintenant

Worktree §0.6, sources §3 en parallèle, attaque D-cont.2.1 (tun_rs Android backend). Vérifier Session G GO avant D.6 DAITA UI.

Bonne route.
