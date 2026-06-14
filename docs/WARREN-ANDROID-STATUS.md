# Warren Android : état, architecture, et chantiers

> Mis a jour : 2026-06-14. Document de reference sur l'etat reel du client
> Warren sur Android : architecture du tunnel, modele wallet/securite, parite
> de features vs le client desktop, et ce qui reste a activer/valider sur
> device. Pendant du `WARREN-MACOS-STATUS.md`. La procedure de test device
> detaillee vit dans `../android/docs/E2E-DEVICE-TEST-CHECKLIST.md`.

## TL;DR

- ✅ **Le build APK passe de bout en bout** (`./gradlew :app:assembleProdDebug`,
  valide 2026-06-14) : `app-prod-debug.apk` avec `libwarren_jni.so` pour les
  4 ABIs (arm64-v8a, armeabi-v7a, x86, x86_64). Tout le Rust (cross-compile
  NDK) + Kotlin + packaging compile et link.
- ⚠️ **Le data-plane du tunnel n'a PAS encore ete verifie sur device.** Un bug
  latent l'aurait fait crasher au connect (plugin kotlinx-serialization absent
  de `:app`, corrige cette session) ; le chemin connect n'a jamais tourne sur
  un vrai appareil. Premier smoke-test device = priorite n°1.
- ✅ **La parite de features avec le desktop est large** (single-hop, DAITA,
  NAT-PMP, allow-LAN, IPv6, MTU, kill-switch, DNS custom + content-blockers,
  picker avec recherche/pays/recents/custom-lists, wallet create/restore/
  backup/erase, subscription + voucher, exit-key pinning).
- 🟡 **Deux features sont "cablees mais inertes"** (multi-hop reel, keystore
  hardware-auth) : le code est ecrit, compile et package, mais l'interrupteur
  d'activation est laisse OFF tant qu'un device n'a pas valide (cf. § Features
  cablees-mais-inertes).

## Architecture (rappel)

Sur Android, **le cycle de vie du tunnel est pilote par Kotlin**, pas par un
daemon en process comme sur desktop :

- `WarrenVpnService` + `WarrenQuinnAdapter` (Kotlin) possedent connect /
  disconnect / reconnect, la boucle de statut, le handover reseau, le
  kill-switch (blackhole-TUN) et le flap-guard.
- `warren-jni` (Rust) = primitives fines : logging, wallet BIP39/Ed25519
  (signature `X-Warren-*`), `run_session` (dial QUIC + pump), relay-list,
  NAT-PMP. C'est `talpid-warren-tunnel` qui joue ce role sur desktop.
- Flot de config : Kotlin construit un `WarrenTunnelConfig` -> JSON -> Intent
  -> `WarrenVpnService` -> `WarrenJni.connectTunnel` -> serde Rust. Les noms
  de champs `@SerialName` (Kotlin) doivent matcher la struct serde (Rust) ;
  un test round-trip (`WarrenTunnelConfigSerializationTest` + tests
  `tunnel.rs::parse_config`) verrouille ce contrat.
- L'enforcement anti-fuite (kill-switch, allow-LAN, DNS in-tunnel, blackhole
  IPv6, MTU, split-tunnel par app) est fait **cote Android** dans
  `WarrenTunInterfacePlan` / `VpnService.Builder`, pas cote Rust.

## Modele wallet / securite (Android)

- **Identite** = wallet BIP39 (12 mots) -> cle Ed25519 -> adresse SS58 `wb…`.
  Une wallet = une pubkey (pas de registre multi-device facon Mullvad).
- **Stockage** : mnemonic chiffre AES-256-GCM par une cle Android Keystore
  (`warren_wallet_master_v1`), ciphertext+IV en `SharedPreferences`
  (`AndroidKeystoreWalletRepository`). La cle ne quitte jamais le Keystore.
- **Gate biometrique** : toute lecture en clair passe par `unlock()`, qui
  exige le `SensitiveOpAuthorizer` (BiometricPrompt) ; `decryptMnemonic` est
  prive et n'a pas d'autre appelant. La mnemonic ne se decrypte jamais sans
  authentification utilisateur (au niveau app).
- **Zeroization** : la mnemonic est un `Mnemonic` (CharArray) zeroizable, pas
  un `String` ; l'adapter tunnel la garde zeroizable le temps de la session
  et la `close()` au teardown (plus de phrase en clair persistante sur le
  heap).
- **Exit-key pinning (TOFU)** : chaque exit voit sa cle epinglee au premier
  usage ; un changement de cle fait echouer le connect (fail-closed), avec un
  bouton "Reset pinned exit keys" pour accepter une rotation legitime.

## Parite de features (vs desktop)

Reference du feature-set upstream : 26 modules `lib/feature/*` au tag
`upstream-baseline-2026-05-06`. Etat Android :

| Domaine | Etat |
|---|---|
| Connect / disconnect / reconnect, etats, chips (QUIC/DAITA/mimicry) | ✅ |
| Wallet : create / restore / backup / erase(=logout) | ✅ |
| Subscription (statut + expiry cache) + voucher + bouton Get subscription | ✅ |
| Picker exit : recherche, groupement pays, recents, custom-lists | ✅ |
| Kill-switch, allow-LAN, DNS custom + content-blockers, IPv6, MTU | ✅ (enforce cote Android) |
| DAITA, NAT-PMP (proto/port/lifetime + statut live) | ✅ |
| Anti-censure (mimicry HTTP/3) | ✅ indicateur read-only (always-on) |
| Exit-key pinning (TOFU) + reset | ✅ |
| Problem report (+ logs natifs via logcat) | ✅ |
| Multi-hop | 🟡 cable-mais-inerte (voir ci-dessous) |
| Keystore hardware-auth (CryptoObject) | 🟡 cable-mais-inerte |
| manage-devices, delete-account serveur | ⚪ hors modele Warren (cf. Non-gaps) |
| Quantum-resistance, obfuscation WireGuard (Shadowsocks/UDP2TCP/LWO) | ⚪ legacy WG, non applicable |

## Features cablees-mais-inertes (activation device-gardee)

Code ecrit + compile + package, mais interrupteur OFF tant que non valide sur
device (activer une feature VPN/wallet sans pouvoir l'executer serait
irresponsable). Procedure detaillee : `../android/docs/E2E-DEVICE-TEST-CHECKLIST.md`.

### Multi-hop reel
- Le data-plane est entierement ecrit : `warren-jni/src/tunnel.rs::run_multi_hop_session`
  (fetch+verif `/v1/multihop/directory`, selection exit + entry distinct,
  `MultiHopClient::connect_with_warren_obfuscation` + `setup_over_stream` +
  `pump_multi_hop_bidirectional`, fail-closed). Le blocker historique (socket
  UDP multi-hop non protege sur Android -> boucle dans le TUN) est leve :
  warren-core `crates/warren-client/src/multi_hop.rs` protege le socket via
  `VpnService.protect` (commit warren-core `6bb7287`).
- **Inerte** : `run_session` branche sur `config.entry_hop`, mais le builder
  Kotlin pose `entryHop = null`. L'UI montre un indicateur read-only
  "non disponible sur Android".
- **Activation** : poser `entryHop` dans `WarrenTunnelConfigBuilder` +
  reactiver le toggle/chip + bumper le pin `.warren-core-version` (pour
  inclure `6bb7287`). Point de vigilance : l'adressage TUN (IpAssign de
  `setup_over_stream` vs TUN fixe `10.66.0.x`).

### Keystore hardware-auth (CryptoObject)
- Chemins DECRYPT (unlock) ET ENCRYPT (create/import) cables via prompt
  `CryptoObject` (cle `setUserAuthenticationRequired`, guards API 30+).
- **Inerte** : flag `HARDWARE_AUTH = false` dans `AndroidKeystoreWalletRepository`.
- **Activation** : passer `HARDWARE_AUTH = true` (seul changement de code) +
  valider sur device (creation de wallet -> prompt biometrique ; tester aussi
  un device sans biometrie enrolee). Les wallets existantes gardent leur cle
  non-auth ; seules les nouvelles cles deviennent auth-required.

## Corrige cette session (2026-06-14)

| Fix | Effet |
|---|---|
| **Plugin kotlinx-serialization manquant sur `:app`** | `Json.encodeToString(WarrenTunnelConfig)` aurait jete `SerializationException` au connect (jamais verifie device). Plugin ajoute ; helpers `toWireJson()`/`warrenTunnelConfigFromWireJson()`. |
| **Multi-hop "fausse promesse"** | Avant : le chip MULTIHOP s'affichait alors que `run_session` dialait en single-hop. Desormais honnete (single-hop) + indicateur read-only ; le multi-hop reel est cable-mais-inerte. |
| **Mnemonic en clair toute la session** | `WarrenQuinnAdapter` gardait la phrase en `String` (heap). Remplace par `Mnemonic` zeroizable, `close()` au teardown. |
| **Code mort** | Suppression des champs config `obfuscation_m40` / `bypass_cidrs` (aucune UI, ignores par Rust) ; clamp MTU clarifie (plafond, lower-only). |
| **Tests** | `KillSwitchPolicy` extrait + teste ; round-trip de config (Kotlin + Rust) ; fix des 4 tests use-case pre-existants (`Intent` non mocke). |

## Non-gaps (volontairement hors modele Warren)

- **manage-devices** : pas de `/v1/devices` (wallet = une pubkey ; sessions
  ephemeres uniquement). Desktop idem. Documente dans `warren-jni/src/android_jni.rs`.
- **delete-account serveur** : couvert par "Erase wallet" (la phrase de
  restauration EST le compte ; rien a supprimer cote serveur).
- **Quantum-resistance / obfuscation WireGuard** : WireGuard-only, sans objet
  pour le tunnel QUIC Warren.

## Prerequis build (env)

- NDK + SDK (cf. `../android/docs/BuildInstructions.macos.md`).
- Le plugin rust-android lance `python` au link. Sur macOS recent seul
  `python3` existe -> ajouter `rust.pythonCommand=/opt/homebrew/bin/python3` a
  `android/local.properties` (deja documente dans `BuildInstructions.macos.md`).
  Sans ca : `python: command not found` (exit 127) au link cargo.

## Chantiers restants (tous device)

Cf. la checklist `../android/docs/E2E-DEVICE-TEST-CHECKLIST.md`. En resume :

1. Smoke-test single-hop : confirmer que le tunnel monte ET que le trafic
   passe (le connect n'a jamais tourne sur device, bug serialization corrige).
2. Activer + valider le multi-hop (flip builder + pin) : verifier le
   data-plane, pas juste "Connected".
3. Activer + valider le keystore hardware (`HARDWARE_AUTH = true`) :
   creation/unlock biometrique + cas sans biometrie.
4. Smoke-test de l'UI non testee unitairement (Get subscription, custom-lists,
   TOFU + reset, logs natifs dans les rapports, tous les toggles tunnel,
   onboarding).
5. Optionnel : faire tourner les tests Rust `parse_config` sur emulateur.
