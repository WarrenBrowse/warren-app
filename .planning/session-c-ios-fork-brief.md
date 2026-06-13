# Session C, iOS fork Mullvad → Warren VPN mobile

> Brief d'agent autonome cross-repo warren-core + warren-app.
> Doctrine §0.0 INVIOLABLE destructive git + §0.5 full autonomy.
> Grosse session séquentielle : l'agent enchaîne C.1 → C.7 sans escalade.

**Effort estimé** : wall-clock 1-2 mois (7 sous-phases).
**Coût Hetzner** : 0 EUR (TestFlight + iOS simulator suffisent).
**Pré-conditions** :
- warren-app `main` HEAD `2c49588b22+` (post-Session B)
- warren-core `main` HEAD `ba819cf+` (post-Session B)
- macOS dev machine recommandé (Xcode 16+ requis pour build iOS)

**Objectif** : livrer Warren VPN iOS bêta TestFlight-ready à partir du fork Mullvad iOS upstream présent dans `warren-app/ios/` et `warren-app/mullvad-ios/`.

Sous-phases (séquentielles autonomes) :

1. **C.1, Rebrand Xcode project + bundle IDs** (~3-5j)
2. **C.2, Rebrand 15 Swift packages** (~5-7j)
3. **C.3, `mullvad-ios` → `warren-ios` crate + wire warren-core** (~7-10j)
4. **C.4, PacketTunnelProvider Quinn (replace WireGuardAdapter)** (~10-14j)
5. **C.5, UI Swift adapt wallet Ed25519 mnemonic auth** (~5-7j)
6. **C.6, Multi-hop + DAITA + NAT-PMP UI parity** (~5-7j)
7. **C.7, Build TestFlight + smoke iOS simulator** (~3-5j)

---

## 0.0 INVIOLABLE, pas de commande git destructive

Cf. doctrine standard (`git stash`, `git checkout <path>`, `git restore`, `git reset --hard`, `git clean` interdits). Préserver tout fichier modified ou untracked. Si état inattendu : escalader sans toucher au tree.

Submodule `ios/wireguard-apple` est VIDE (uninit). Si l'agent décide d'init pour récup le WG code, escalader d'abord. Sinon, Warren remplace WG par Quinn, ce submodule probablement non requis.

Violation = scope error CRITIQUE. Incident M4.H.F 2026-05-20 : 5 fichiers WIP poka perdus warren-core.

---

## 0.5 MANDAT D'AUTONOMIE

Cf. memory `feedback_agent_full_autonomy_no_timid_rollback`. Plein mandat. Diagnostic 30 min → fix tactique TDD → commit + push → reprise. PAS de rollback timide.

Escalade `AskUserQuestion` SEULEMENT si :
1. Secret leak (Apple Developer keys, .p12 certs, App Store Connect API key, mnemonic)
2. Coût > 0.30 EUR (n/a hors infra dev)
3. Breaking change /v1 wire format
4. Signing key prod (.p12 macOS, provisioning profiles iOS)
5. **Spécifique session C** : si tu détectes nécessité changer warren-core wire format pour adapter mobile constraints, escalade avant push

Décisions tactiques agent autorisées :
- Choisir Swift Package Manager vs CocoaPods (recommandation SPM, déjà utilisé Mullvad)
- Bundle ID convention (recommandation `com.warrenbrowse.vpn.ios` + `com.warrenbrowse.vpn.ios.PacketTunnel` extension)
- Logo iOS = SVG generate from `desktop/packages/mullvad-vpn/graphics/icon.svg` (taupe Warren rebrand R1) ou placeholder W jaune navy si conversion bloquée
- Localizable.xcstrings : FR + EN minimum, autres locales Mullvad → migrer tel quel
- TestFlight invite-only beta channel par défaut
- Targeting iOS 16+ (vs iOS 15 Mullvad) pour réduire surface compat, escalade si tu vises plus bas

---

## 1. Setup initial

```bash
cd /Users/poka/dev/warrenBros/warren-app
git status                                  # clean main HEAD 2c49588b22+
git remote -v                                # origin = github.com/WarrenBrowse/warren-app
cd /Users/poka/dev/warrenBros/warren-core
git status                                  # clean ba819cf+
```

Build pré-flight (sur macOS) :
```bash
cd /Users/poka/dev/warrenBros/warren-app/ios
ls MullvadVPN.xcodeproj                      # confirme Xcode project présent
xcodebuild -list -project MullvadVPN.xcodeproj # confirme targets list
```

Si Xcode pas installé : escalade. Si non-macOS : skip C.7 build TestFlight, livre code path complet + tests.

---

## 2. Optimisations agent

- Read sources cross-repo + Swift + Xcode config en PARALLÈLE
- Tests TDD groupés en fin de sous-phase
- Push warren-core + warren-app au fil de l'eau
- Pin warren-core bump si nouveaux commits warren-core requis (rare en session C, warren-core déjà mobile-able via path-dep workspace)
- Rebrand utilise `sed -i` opportuniste mais vérifier qu'on ne casse pas de string literals dans Swift code (string `"Mullvad"` peut être user-facing valide à garder, vs identifier `Mullvad` à rebrand). Lire avant sed.

---

## C.1, Rebrand Xcode project + bundle IDs (~3-5j)

### Scope C.1

1. **C.1.1** Rename `ios/MullvadVPN.xcodeproj` → `ios/WarrenVPN.xcodeproj` (Xcode → File → Rename, ou édition XML pbxproj). Cohérence projet + scheme + targets.
2. **C.1.2** Bundle IDs (rebrand 4 targets) :
   - App main : `net.mullvad.MullvadVPN` → `com.warrenbrowse.vpn.ios`
   - PacketTunnel extension : `net.mullvad.MullvadVPN.PacketTunnel` → `com.warrenbrowse.vpn.ios.PacketTunnel`
   - Notifications extension : idem suffix `.Notifications`
   - Tests : suffix `.Tests`
3. **C.1.3** App Group `group.net.mullvad.MullvadVPN` → `group.com.warrenbrowse.vpn` (utilisé par App ↔ Extension shared storage)
4. **C.1.4** Display name `Mullvad VPN` → `Warren VPN` dans Info.plist (CFBundleDisplayName + CFBundleName)
5. **C.1.5** App icon : remplacer `Assets/AppIcon.appiconset/` par Warren logo (W jaune `#ffd524` sur navy `#0a1422`, cohérent warrenbrowse.com + desktop)
6. **C.1.6** Launch screen : Warren-branded
7. **C.1.7** Configurations/*.xcconfig : adapter TEAM_ID si présent (escalade poka pour Apple Developer Team ID), PRODUCT_BUNDLE_IDENTIFIER, DEVELOPMENT_TEAM, CODE_SIGN_STYLE Automatic + identity Apple Developer
8. **C.1.8** Tests TDD : `xcodebuild -list -project WarrenVPN.xcodeproj` retourne les targets renommées, scheme builds OK (sans signing pour CI mode)

### Critères GO C.1

- Xcode project ouvre dans Xcode sans erreur
- 4 bundle IDs cohérents
- App Group + Display Name + Icon Warren
- Build target `WarrenVPN` PASS (mode Debug, signing désactivé)

### Décisions tactiques C.1

- TEAM_ID : escalade poka (Apple Developer account info)
- Apple Developer account info absent → continue avec TEAM_ID placeholder `XXXXXXXXXX` + bandeau caveat dans report
- App Group : `group.com.warrenbrowse.vpn`
- Min iOS 16

---

## C.2, Rebrand 15 Swift packages (~5-7j)

### Scope C.2

Renommer module-by-module :

| Mullvad package | Warren package |
|-----------------|----------------|
| MullvadVPN | WarrenVPN |
| MullvadREST | WarrenREST |
| MullvadRESTTests | WarrenRESTTests |
| MullvadSettings | WarrenSettings |
| MullvadTypes | WarrenTypes |
| MullvadLogging | WarrenLogging |
| MullvadMockData | WarrenMockData |
| MullvadPostQuantum | (drop, Warren n'utilise pas PQ WG legacy) |
| MullvadRustRuntime | WarrenRustRuntime |
| MullvadRustRuntimeTests | WarrenRustRuntimeTests |
| Operations | (garder, générique) |
| OperationsTests | (garder) |
| PacketTunnel | (garder, target name, contenu rebrand) |
| PacketTunnelCore | (garder, contenu rebrand) |
| MullvadVPNTests | WarrenVPNTests |
| MullvadVPNUITests | WarrenVPNUITests |

Pour chaque package :
1. Renommer Package.swift name attribute
2. Renommer Sources/<Module>/ folder
3. `import Mullvad*` → `import Warren*` dans tous les .swift files (rg + sed)
4. Renommer types `MullvadFoo` → `WarrenFoo` si Warren-specific, garder `MullvadFoo` si import upstream WG-legacy (rare ; ex: `MullvadEndpoint` → `WarrenEndpoint`)
5. Update Xcode project references

### Critères GO C.2

- Tous les packages renommés
- Imports cohérents
- `xcodebuild build -scheme WarrenVPN` PASS (mode Debug)
- Pas de string literal `Mullvad` dans UI user-facing (Localizable.xcstrings)

### Décisions tactiques C.2

- Stratégie sed : test sur 1 package, valider, puis appliquer aux 14 autres en parallèle
- Pas de rebrand des dépendances upstream (talpid-*, mullvad-api) si pas Warren-specific
- Drop `MullvadPostQuantum` package : Warren utilise Quinn cleartext + HPKE multi-hop, pas WireGuard PostQuantum tunnel
- `MullvadRustRuntime` → `WarrenRustRuntime` : ce package est le pont Swift → mullvad-ios Rust, devient pont Swift → warren-ios Rust (cf. C.3)

---

## C.3, `mullvad-ios` → `warren-ios` crate + wire warren-core (~7-10j)

### Scope C.3

1. **C.3.1** Renommer crate `mullvad-ios` → `warren-ios` (Cargo.toml name + path workspace)
2. **C.3.2** Réécrire API client : `mullvad_api` calls → `warren_api_client` calls. Le crate `warren-api-client` existe warren-core, expose HTTP signature canonical_message (M4.H.C.PRE refactor). Wire FFI exports vers warren-api-client à la place de mullvad-api.
3. **C.3.3** Drop modules non-pertinents :
   - `ephemeral_peer_proxy/` (Mullvad WireGuard ephemeral peer, n/a Warren Quinn HPKE)
   - `wireguard_key.rs` (WG key gen, n/a Warren Ed25519 wallet)
4. **C.3.4** Ajouter modules Warren :
   - `warren_tunnel_ffi.rs` : FFI export warren-tunnel `WarrenTunnelParameters` + connect/disconnect handles
   - `warren_wallet_ffi.rs` : FFI export warren-identity BIP39 mnemonic generation + signing
   - `warren_multihop_ffi.rs` : FFI export warren-multihop HPKE handshake
   - `warren_natpmp_ffi.rs` : FFI export warren-natpmp-client port-forwarding
5. **C.3.5** cbindgen update : générer header `WarrenIOS.h` à la place de `MullvadIOS.h`. Output path : `ios/WarrenRustRuntime/Sources/WarrenRustRuntime/include/`
6. **C.3.6** Swift bindings dans `WarrenRustRuntime/Sources/WarrenRustRuntime/` : créer Swift wrappers idiomatiques au-dessus du C header. Pattern Mullvad existant à adapter.
7. **C.3.7** Tests Rust : tests existants `mullvad-ios/tests/` adapter pour `warren-ios`, garder couverture API + FFI

### Critères GO C.3

- Crate `warren-ios` compile pour `target_os = "ios"` (cargo build --target aarch64-apple-ios + aarch64-apple-ios-sim)
- Header `WarrenIOS.h` généré valide
- Swift `WarrenRustRuntime` consomme le header sans erreur
- Tests Rust PASS

### Décisions tactiques C.3

- cargo lipo / cargo-xcode integration : utiliser scripts Mullvad existants dans `ios/build-rust.sh` (ou équivalent), rebrand
- staticlib pour iOS (vs cdylib Android), ne pas changer
- `tunnel-obfuscation` Mullvad dep : drop (Warren utilise obfuscation M4.0 HTTP/3 mimicry intégrée dans warren-tunnel)
- `shadowsocks` dep : drop (Mullvad-specific bridge protocol)

---

## C.4, PacketTunnelProvider Quinn (replace WireGuardAdapter) (~10-14j)

### Scope C.4

C'est la sous-phase la plus complexe. NetworkExtension iOS PacketTunnelProvider doit :
- Accepter VPN config depuis App main
- Établir tunnel via warren-tunnel Quinn (vs WireGuard adapter Mullvad)
- Router IP packets entre TUN virtual (NEPacketTunnelFlow) et warren-tunnel
- Reconnect on network change (handover Wi-Fi ↔ cellular)
- Killswitch on connection lost

1. **C.4.1** Étudier `ios/PacketTunnel/PacketTunnelProvider/` Mullvad WireGuard impl
2. **C.4.2** Remplacer `WireGuardAdapter` import par `WarrenRustRuntime` adapter
3. **C.4.3** Implémenter `WarrenQuinnAdapter` Swift class :
   - `init(packetFlow: NEPacketTunnelFlow)` 
   - `startTunnel(config: WarrenTunnelConfig)` async throws
   - `stopTunnel()` async
   - `handleNetworkChange()` notification handler
   - Internal : spawn Quinn via warren-tunnel FFI, route packets via packetFlow.readPackets / writePackets
4. **C.4.4** Config translation : `NEVPNProtocolWarren` (custom subclass NETunnelProvider) → `WarrenTunnelConfig` Rust struct. Inclut : exit pubkey, exit IP:port, wallet signing key, multi-hop relay config (optional), DAITA spec (optional), NAT-PMP enabled (optional), bypass_cidrs (optional)
5. **C.4.5** Killswitch : on tunnel down, NetworkExtension automatically blocks traffic. Verify iOS configuration "Disconnect on Demand" rules ne casse pas Warren multi-hop.
6. **C.4.6** Reconnect : auto-reconnect handler avec Backoff::HANDSHAKE 15s (cf. M4.H.G warren-core), event vers App main via App Group shared UserDefaults
7. **C.4.7** Tests : tests Swift `PacketTunnelTests` + tests Rust FFI warren-ios

### Critères GO C.4

- PacketTunnelProvider connect tunnel Warren OK sur iOS simulator
- Disconnect OK
- Network change handover sans drop tunnel
- Killswitch active automatique
- DNS leak test PASS

### Décisions tactiques C.4

- DROP MullvadPostQuantum integration : Warren utilise HPKE multi-hop, pas WG PQ tunnel
- DROP WireGuardKit dep : Warren utilise Quinn, pas WG
- KEEP NEPacketTunnelFlow router pattern : compat iOS standard

---

## C.5, UI Swift adapt wallet Ed25519 mnemonic auth (~5-7j)

### Scope C.5

Mullvad iOS auth = account number 16 digits. Warren iOS auth = wallet Ed25519 + BIP39 mnemonic (parité desktop session B onboarding).

1. **C.5.1** Login screen : remplacer `AccountNumberInput` par `MnemonicInput` (12-word BIP39 paste/type)
2. **C.5.2** Signup wizard mobile : 5-step similaire desktop session B :
   - Welcome
   - Wallet (Generate avec 12 mots BIP39 display blur+reveal OR Import 12 mots)
   - Subscription (placeholder "Visit warrenbrowse.com to subscribe", lien WebView)
   - Privacy preferences (multi-hop, DAITA, obfuscation toggles)
   - Done
3. **C.5.3** Wallet storage : iOS Keychain pour mnemonic + signing key (vs UserDefaults desktop). Pattern Mullvad existant pour account number à adapter.
4. **C.5.4** Restore flow : Settings → "Restore wallet" → MnemonicInput
5. **C.5.5** Backup CTA : "View mnemonic" Settings, password-protected (Face ID / Touch ID)
6. **C.5.6** Mnemonic display security : pas de CTA "Copier" (anti-malware-clipboard), blur+reveal pattern desktop
7. **C.5.7** i18n FR + EN minimum (Localizable.xcstrings)

### Critères GO C.5

- Login + Signup wizard fonctionnels
- Wallet Keychain storage
- Restore + Backup flows
- i18n FR+EN
- Tests UI Swift `WarrenVPNUITests` PASS

### Décisions tactiques C.5

- Keychain Class : kSecClassGenericPassword + kSecAttrAccessibleWhenUnlockedThisDeviceOnly
- Face ID / Touch ID : LocalAuthentication framework, fallback passcode
- No iCloud sync mnemonic : strictement device-local

---

## C.6, Multi-hop + DAITA + NAT-PMP UI parity (~5-7j)

### Scope C.6

Parité avec desktop M4.H.C + session B :

1. **C.6.1** Multi-hop view iOS : toggle entry country + exit country pickers (équivalent `WarrenMultiHopSettingsView.tsx` desktop)
2. **C.6.2** DAITA toggle iOS : settings section privacy, tooltip overhead 10%
3. **C.6.3** Obfuscation M4.0 indicator : connection-details view "HTTP/3 mimicry active"
4. **C.6.4** NAT-PMP port-forwarding settings : enabled toggle + display port forwarded + lifetime
5. **C.6.5** Multi-exit failover : status banner "Switched to <country>"
6. **C.6.6** Country picker : utilise warren-relay-selector `WarrenRelayList` exposé via FFI warren-ios
7. **C.6.7** Settings → Privacy preferences (parité desktop)

### Critères GO C.6

- Multi-hop opérationnel iOS (entry + exit pickers)
- DAITA toggle wire + applique
- Obfuscation indicator visible
- NAT-PMP UI fonctionnelle (port affichage)
- Failover banner

### Décisions tactiques C.6

- DAITA default OFF (memory `warren_daita_doctrine_v1`)
- Multi-hop default OFF
- Obfuscation always-on (M4.0 baseline)

---

## C.7, Build TestFlight + smoke iOS simulator (~3-5j)

### Scope C.7

1. **C.7.1** Build Release iOS pour iPhone simulator + iPad simulator + iOS device
2. **C.7.2** Smoke iOS simulator :
   - Onboarding wizard complete generate wallet
   - Connect Warren exit-1 prod
   - DNS leak test (curl ifconfig.me + dig)
   - Multi-hop toggle + reconnect
   - DAITA toggle apply
   - NAT-PMP qBittorrent simulator (si feasible)
   - Disconnect + reconnect
3. **C.7.3** Bundle App Store metadata (App Store Connect API ou fastlane) : title, subtitle, description (FR+EN), keywords, screenshots iPhone 6.7"+5.5", privacy nutrition label
4. **C.7.4** TestFlight invite-only group : config dans App Store Connect (escalade poka pour Apple Developer account access)
5. **C.7.5** Upload .ipa to TestFlight via `xcrun altool` ou `xcrun notarytool` (idem signing key gen pending = case 4 escalation)

### Critères GO C.7

- Build Release iOS PASS
- 6-7 smoke tests simulator PASS
- App Store metadata complète (peut être placeholder text si content business pending poka)
- TestFlight upload skip si signing key pending poka (pas un blocker)

### Décisions tactiques C.7

- Sans Apple Developer account / signing : build PASS suffit, TestFlight upload skip → escalation caveat report
- Screenshots : génération automatique via fastlane snapshot recommandée

---

## 3. Sources cross-repo à lire (PARALLÈLE)

### warren-app iOS surface
- `ios/MullvadVPN.xcodeproj/project.pbxproj` (config Xcode)
- `ios/Configurations/*.xcconfig` (signing + bundle config)
- `ios/MullvadVPN/Package.swift` (15 Swift packages)
- `ios/PacketTunnel/PacketTunnelProvider/` (NetworkExtension entry)
- `ios/MullvadRustRuntime/Sources/MullvadRustRuntime/include/MullvadIOS.h` (FFI header)
- `mullvad-ios/Cargo.toml` + `mullvad-ios/src/lib.rs` (Rust FFI entry)
- `mullvad-ios/src/api_client/` (API patterns)

### warren-core
- `crates/warren-tunnel/src/lib.rs` (target FFI)
- `crates/warren-api-client/src/lib.rs` (target FFI API)
- `crates/warren-identity/src/lib.rs` (BIP39 + Ed25519 wallet)
- `crates/warren-multihop/src/lib.rs` (HPKE)
- `crates/warren-natpmp-client/src/lib.rs` (port-forwarding)
- `crates/warren-relay-selector/src/lib.rs` (country picker)

### Référence Mullvad
- `MullvadRustRuntime/Sources/MullvadRustRuntime/` Swift wrappers existants
- `PacketTunnel/PacketTunnelProvider/` WireGuard adapter pattern

---

## 4. Plan d'exécution (séquentiel, autonome)

```
C.1 Rebrand Xcode (3-5j)
C.2 Rebrand Swift packages (5-7j)
C.3 warren-ios crate + wire (7-10j)
C.4 PacketTunnelProvider Quinn (10-14j) ← phase la plus complexe
C.5 UI wallet (5-7j)
C.6 UI multi-hop/DAITA/NAT-PMP parity (5-7j)
C.7 Build TestFlight + smoke (3-5j)
C.8 Rapport final (1h)
  └── .planning/session-c-report.md avec verdict GO ULTIMATE par sous-phase
```

Total ~1.5-2 mois wall-clock. Push warren-core (si modif) + warren-app au fil de l'eau, ne pas batch 50+ commits en fin de session.

---

## 5. Critères GO ULTIMATE session C

- ✅ C.1-C.7 critères GO PASS individuels
- ✅ `xcodebuild build -scheme WarrenVPN -destination 'platform=iOS Simulator'` PASS
- ✅ `cargo test --workspace` warren-core + warren-app PASS (pas de régression desktop)
- ✅ `cargo clippy --workspace --all-targets -- -D warnings` PASS
- ✅ `cargo build --target aarch64-apple-ios -p warren-ios` PASS
- ✅ `cargo build --target aarch64-apple-ios-sim -p warren-ios` PASS
- ✅ Pas de régression Linux/Mac/Win desktop : connect/disconnect OK
- ✅ Rapport `.planning/session-c-report.md` rédigé

Verdict GO PARTIEL acceptable si :
- TestFlight upload skipped (signing key pending poka)
- iOS device smoke skipped (pas d'iPhone réel dispo)
- C.7 smoke simulator PASS suffit pour GO PARTIEL

Verdict NO-GO si Xcode build refuse de compiler malgré §0.5 autonomy exhausted.

---

## 6. Doctrine

- §0.0 INVIOLABLE git
- §0.5 autonomy
- English-only code comments (Swift `//`, Rust `//`)
- Pas em-dash
- Pas secrets in commits (Apple Developer keys, .p12, signing identity)
- 5 concurrents comparison standard quand pertinent
- Pas Cure53 mention

---

## 7. Memory updates attendus

- `warren_session_c_delivered.md`, verdict + caveats par sous-phase
- Update `MEMORY.md` index
- Memory dédié si feature mobile non-triviale (ex: `warren_ios_packettunnel_adapter.md`)

---

## 8. Commencer maintenant

Lis le brief en entier, source §3 en parallèle, attaque C.1.1. Plein mandat §0.5. Push au fil de l'eau. iOS = audience critique (50% VPN payant), priorité produit.

Bonne route.
