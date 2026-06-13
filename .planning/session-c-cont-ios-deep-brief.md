# Session C-continuation, iOS C.3 deep + C.4-C.7

> Brief d'agent autonome warren-app + warren-core path-deps.
> Doctrine §0.0 INVIOLABLE destructive git + §0.5 full autonomy + §0.6 WORKTREE SÉPARÉ OBLIGATOIRE.
> Continuation Session C partielle (C.1 + C.2 + C.3 skeleton livrés 2026-05-21).

**Effort estimé** : wall-clock 20-35 jours (5 sous-phases restantes).
**Coût Hetzner** : 0 EUR (TestFlight + iOS simulator suffisent).
**Pré-conditions** :
- warren-app `main` HEAD post-Session-G (DAITA pump fix livré, mandatory pour DAITA UI C.6)
- warren-core `main` HEAD post-Session-G (pump_with_daita stable)
- Session C report : `.planning/session-c-report.md` (si présent) + `warren_session_c_c1_delivered.md` memory
- macOS dev machine avec Xcode 16+

**Objectif** : compléter le fork iOS Mullvad → Warren VPN mobile, livrer TestFlight-ready bêta.

Sous-phases restantes (séquentielles autonomes) :

1. **C-cont.1, Setup worktree warren-app dédié iOS** (~30 min)
2. **C.3 deep, FFI rewrite warren-ios crate + cargo build iOS targets** (~5-7j)
3. **C.4, PacketTunnelProvider Quinn (replace WireGuardAdapter)** (~10-14j)
4. **C.5, UI Swift wallet Ed25519 BIP39** (~5-7j)
5. **C.6, Multi-hop + DAITA + NAT-PMP UI parity** (~5-7j, dépend Session G livré)
6. **C.7, Build TestFlight + smoke iOS simulator** (~3-5j)

---

## 0.0 INVIOLABLE, pas de commande git destructive

Cf. doctrine standard. Préserver fichiers modified/untracked. Incident M4.H.F = 5 fichiers WIP poka perdus.

---

## 0.5 MANDAT D'AUTONOMIE

Plein mandat. Escalade SEULEMENT si secret leak, coût > 0.30 EUR, breaking /v1, signing key prod, OU **spécifique session C-cont** : si DAITA pump warren-core encore instable (vérifier Session G GO ULTIMATE avant câbler DAITA UI C.6), escalader pour valider readiness mobile.

---

## 0.6 WORKTREE SÉPARÉ OBLIGATOIRE (CRITIQUE)

Incident 2026-05-21 Sessions C + D parallèles même worktree → commit D a absorbé git mv iOS C. Cette continuation ne doit JAMAIS partager worktree avec D-cont ou autres sessions.

```bash
cd /Users/poka/dev/warrenBros/warren-app
git fetch origin
git worktree add ../warren-app-ios-c-cont main
cd ../warren-app-ios-c-cont
git status                                  # clean main post-G
```

Tous les commits + push depuis ce worktree. Cleanup en fin :
```bash
cd /Users/poka/dev/warrenBros/warren-app
git worktree remove ../warren-app-ios-c-cont
```

NE PAS travailler dans `/Users/poka/dev/warrenBros/warren-app` directement (race garantie si autres sessions actives).

---

## 1. Setup initial

```bash
cd /Users/poka/dev/warrenBros/warren-app
git fetch origin
git worktree add ../warren-app-ios-c-cont main
cd ../warren-app-ios-c-cont

git log --oneline -10                       # confirme C.1+C.2+C.3 skeleton + Session G présent
ls ios/WarrenVPN.xcodeproj                  # confirme rebrand Session C
ls warren-ios                                # confirme skeleton Session C
```

Lire artifacts Session C :
```bash
cat /Users/poka/.claude/projects/-Users-poka-dev-warrenBros-warren-app/memory/warren_session_c_c1_delivered.md
```

---

## 2. Optimisations agent

- Cross-compile cargo --target aarch64-apple-ios + aarch64-apple-ios-sim en parallèle
- Tests Swift `xcodebuild test` groupés en fin de sous-phase
- Push warren-app au fil de l'eau
- Pas re-build full Xcode après chaque edit Swift (incremental build)

---

## C-cont.1, Setup worktree (~30 min)

Cf. §0.6.

### Critères GO C-cont.1

- Worktree opérationnel
- HEAD warren-app post-Session-G confirmé
- Artifacts Session C lus

---

## C.3 deep, FFI rewrite warren-ios crate + cargo build iOS targets (~5-7j)

### Contexte

Session C livré skeleton C.3 (crate `warren-ios` renamed + Swift `WarrenRustRuntime` + header `warren_rust_runtime.h` + modulemap + build.rs cbindgen). RESTE :
- FFI rewrite : drop `mullvad-api` deps → wire `warren-api-client`
- Drop modules WG-legacy (`ephemeral_peer_proxy`, `wireguard_key`)
- Add 4 warren-specific FFI modules
- cargo build iOS targets PASS

### Scope C.3 deep

1. **C.3.1** Update `warren-ios/Cargo.toml` :
   - Drop `mullvad-api`, `mullvad-encrypted-dns-proxy`, `mullvad-logging`, `mullvad-types`, `shadowsocks`, `talpid-tunnel-config-client`, `tunnel-obfuscation`
   - Add path-deps warren-core : `warren-api-client`, `warren-tunnel`, `warren-client`, `warren-multihop`, `warren-natpmp-client`, `warren-identity`, `warren-relay-selector`, `warren-protocol`
2. **C.3.2** Drop `warren-ios/src/wireguard_key.rs` + `warren-ios/src/ephemeral_peer_proxy/` (modules WG ephemeral)
3. **C.3.3** Rewrite `warren-ios/src/api_client/mod.rs` : adapter API calls vers `warren-api-client` (canonical_message HTTP signature, /v1/exits, /v1/subscribers/*, /v1/incidents)
4. **C.3.4** Add 4 nouveaux modules FFI :
   - `warren-ios/src/warren_tunnel_ffi.rs` : exports `WarrenTunnelParameters` + connect/disconnect handles + status events
   - `warren-ios/src/warren_wallet_ffi.rs` : exports BIP39 mnemonic generate/import + Ed25519 sign
   - `warren-ios/src/warren_multihop_ffi.rs` : exports HPKE handshake + relay selection
   - `warren-ios/src/warren_natpmp_ffi.rs` : exports NAT-PMP port-forwarding
5. **C.3.5** Update `warren-ios/build.rs` cbindgen config : génère `warren_rust_runtime.h` avec les nouveaux exports
6. **C.3.6** Cross-compile iOS targets :
   ```bash
   cargo build --target aarch64-apple-ios -p warren-ios --release
   cargo build --target aarch64-apple-ios-sim -p warren-ios --release
   cargo build --target x86_64-apple-ios -p warren-ios --release  # legacy sim Intel Mac
   ```
7. **C.3.7** lipo fat binary :
   ```bash
   lipo -create \
     target/aarch64-apple-ios/release/libwarren_ios.a \
     target/aarch64-apple-ios-sim/release/libwarren_ios.a \
     -output ios/WarrenRustRuntime/Sources/WarrenRustRuntime/libwarren_ios.a
   ```
8. **C.3.8** Update Swift `WarrenRustRuntime/Sources/WarrenRustRuntime/` wrappers idiomatiques au-dessus du nouveau header (drop WG-specific wrappers, add warren-specific)
9. **C.3.9** Tests Rust : `cargo test -p warren-ios` PASS
10. **C.3.10** Tests Swift : `xcodebuild test -scheme WarrenRustRuntimeTests` PASS

### Critères GO C.3 deep

- Crate `warren-ios` compile pour 3 iOS targets PASS
- Header `warren_rust_runtime.h` régénéré valide
- lipo fat binary produit
- Swift wrappers Warren-specific
- Tests Rust + Swift PASS

---

## C.4, PacketTunnelProvider Quinn (replace WireGuardAdapter) (~10-14j)

Phase la plus complexe. Cf. brief original Session C §C.4 pour scope détaillé. Résumé :

1. Remplacer `WireGuardAdapter` import Mullvad par `WarrenRustRuntime`
2. Implémenter `WarrenQuinnAdapter` Swift : init/startTunnel/stopTunnel/handleNetworkChange
3. Config translation `NEVPNProtocolWarren` → `WarrenTunnelConfig` Rust
4. Killswitch automatique via NetworkExtension
5. Reconnect Backoff::HANDSHAKE 15s
6. Tests Swift + Rust FFI

### Critères GO C.4

- PacketTunnelProvider connect Warren tunnel iOS simulator OK
- Disconnect OK
- Network change handover sans drop
- Killswitch active
- DNS leak test PASS

### Décisions tactiques C.4

- DROP `MullvadPostQuantum` (Warren utilise HPKE multi-hop)
- DROP `WireGuardKit` dep (Warren utilise Quinn)
- KEEP NEPacketTunnelFlow router pattern

---

## C.5, UI Swift wallet Ed25519 BIP39 (~5-7j)

Cf. brief original Session C §C.5. Résumé :

1. Login screen : `MnemonicInput` (12-word BIP39 paste/type)
2. Signup wizard mobile 5-step (parité desktop session B onboarding)
3. Wallet storage : iOS Keychain
4. Restore + Backup flows
5. Mnemonic blur+reveal sans clipboard CTA
6. i18n FR + EN

### Critères GO C.5

- Login + Signup wizard fonctionnels iOS
- Keychain storage encrypted
- Backup gated Face ID / Touch ID
- i18n FR+EN
- Tests UI PASS

---

## C.6, Multi-hop + DAITA + NAT-PMP UI parity (~5-7j, dépend Session G)

⚠️ **DÉPENDANCE STRICTE** : Session G `pump_with_daita` stability fix LIVRÉ + verdict GO ULTIMATE avant attaquer C.6 DAITA UI. Sinon DAITA UI iOS = feature non-shippable (bug prod connu Session F).

Cf. brief original Session C §C.6. Résumé :

1. Multi-hop view iOS : entry + exit country pickers
2. DAITA toggle iOS settings privacy
3. Obfuscation M4.0 indicator
4. NAT-PMP port-forwarding settings
5. Multi-exit failover status banner
6. Country picker via warren-relay-selector FFI

### Critères GO C.6

- Multi-hop opérationnel
- DAITA toggle applique (post-Session G validé)
- Obfuscation indicator visible
- NAT-PMP UI fonctionnelle
- Failover banner

### Décisions tactiques C.6

- Si Session G NO-GO : DAITA UI iOS = feature flagged OFF + hidden Settings
- Multi-hop + NAT-PMP + obfuscation indépendants Session G, peuvent ship

---

## C.7, Build TestFlight + smoke iOS simulator (~3-5j)

Cf. brief original Session C §C.7. Résumé :

1. Build Release iOS
2. Smoke iOS simulator (7-8 tests)
3. App Store metadata
4. TestFlight upload (skip si signing pending poka)

### Critères GO C.7

- Build Release PASS
- 7-8 smoke tests simulator PASS
- App Store metadata complète
- TestFlight skip OK si signing pending

---

## 3. Sources cross-repo à lire (PARALLÈLE)

### warren-app iOS surface (post-Session C skeleton)
- `ios/WarrenVPN.xcodeproj/project.pbxproj`
- `ios/Configurations/*.xcconfig`
- `ios/WarrenVPN/Package.swift` (9 Swift packages renamed Session C.2)
- `ios/PacketTunnel/PacketTunnelProvider/`
- `ios/WarrenRustRuntime/Sources/WarrenRustRuntime/include/warren_rust_runtime.h`
- `warren-ios/Cargo.toml` + `warren-ios/src/lib.rs` (post-Session C.3 skeleton)
- `warren-ios/build.rs` (cbindgen)

### warren-core path-deps
- `crates/warren-tunnel/src/lib.rs`
- `crates/warren-api-client/src/lib.rs`
- `crates/warren-identity/src/lib.rs`
- `crates/warren-multihop/src/lib.rs`
- `crates/warren-natpmp-client/src/lib.rs`
- `crates/warren-relay-selector/src/lib.rs`

### Référence Mullvad
- `mullvad-ios/` legacy upstream (lire pour patterns FFI sans renommer)
- `MullvadRustRuntime/Sources/MullvadRustRuntime/` Swift wrappers patterns

---

## 4. Plan d'exécution (séquentiel)

```
C-cont.1 Worktree setup (30 min)
C.3 deep FFI rewrite + iOS targets build (5-7j)
C.4 PacketTunnelProvider Quinn (10-14j) ← le plus complexe
C.5 UI wallet BIP39 (5-7j)
C.6 UI multi-hop/DAITA/NAT-PMP (5-7j, post-Session-G)
C.7 TestFlight smoke (3-5j)
C.8 Rapport + memory + cleanup
```

Total ~20-35j wall-clock.

---

## 5. Critères GO ULTIMATE session C-cont

- ✅ C.3 deep + C.4 + C.5 + C.6 + C.7 critères GO PASS
- ✅ `xcodebuild build -scheme WarrenVPN -destination 'platform=iOS Simulator'` PASS
- ✅ `cargo build --target aarch64-apple-ios -p warren-ios` PASS
- ✅ `cargo build --target aarch64-apple-ios-sim -p warren-ios` PASS
- ✅ `cargo test --workspace` warren-core + warren-app PASS (pas régression desktop)
- ✅ Worktree cleaned

Verdict GO PARTIEL acceptable si :
- TestFlight upload skipped (signing pending poka case 4)
- iOS device smoke skipped (pas iPhone réel dispo)
- C.6 DAITA UI feature-flagged OFF si Session G pas GO ULTIMATE

---

## 6. Doctrine

- §0.0 INVIOLABLE git
- §0.5 autonomy
- §0.6 worktree séparé obligatoire (CRITIQUE)
- English-only code comments
- Pas em-dash
- Pas secrets in commits

---

## 7. Memory updates attendus

- `warren_session_c_cont_delivered.md`
- Update MEMORY.md

---

## 8. Commencer maintenant

Worktree §0.6, sources §3 en parallèle, attaque C.3.1. Vérifier Session G GO avant C.6 DAITA UI.

Bonne route.
