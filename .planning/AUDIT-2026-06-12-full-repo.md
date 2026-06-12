# Audit complet warren-app : 2026-06-12

## APPLIQUÉ (même session, 2026-06-12)

Vérifié : `cargo check` vert sur daemon + talpid-warren-tunnel + cli +
management-interface + problem-report ; desktop `tsc --noEmit` 0 erreur.

- **H3** patch `quinn-udp` dans `[patch.crates-io]` (le fork s'applique
  maintenant à warren-tunnel/-client/-relay ; vérifié `cargo tree -i quinn-udp`
  = copie unique forkée).
- **M15** `[profile.release.package]` opt-level 3 sur warren-tunnel/-client/
  -relay/-multihop/-protocol/-tls + chacha20poly1305 + hpke.
- **H4** `assert_warren_core_pin` dans build.sh (gate release/signed : HEAD
  warren-core == `.warren-core-version` + arbre propre).
- **M6** `u64::try_from(...).unwrap_or(0)` (account_backend.rs).
- **M7** `lock().unwrap_or_else(|p| p.into_inner())` x3 (pump_error_tx,
  lib.rs + migration_watchdog.rs).
- **M8** commentaire ABA corrigé. **M9** warning plaintext-storage.
  **M12** `expect` reqwest x2 (perte de pinning évitée). **M13** warning
  custom-list fallback. **M16** tick pump_metrics 2s -> 30s.
- CLAUDE.md : narration « Step 1/2/3 », tombstones « M-1 fix », dividers
  box-drawing U+2500, commentaire FR du .service traduits/supprimés.
- Desktop **M1-M5** (agent, tsc vert) : clipboard auto-clear côté main process,
  backup-pending persisté (gui-settings), guard createAccountFailed, token retiré
  des URLs checkout, UUID-validate savedReportId.
- CLI rebrand user-facing « Mullvad »->« Warren » (help text only),
  proto FR->EN + correction du commentaire SetWarrenMnemonic (hot-swap),
  chemins logs problem-report `Mullvad VPN`->`Warren VPN`.
- CI : warren-checks sur `pull_request` (paths-filtered + concurrency cancel,
  PAS push:main), jobs cassés réparés (daemon.yml token Linux, clippy.yml
  `mullvad-jni`->`warren-jni`, rust-supply-chain token), osv-scanner-scheduled
  réactivé (cron). Heavy matrices restent dispatch-only.
- **osv** ignore RUSTSEC-2025-0141 prolongé (2026-06-07 expiré -> 2026-12-07).
- masque-proxy : NON supprimé. README.warren.md (référence MASQUE) +
  `.planning/PRODUCT-obfuscation-transport.md` (besoin produit obfuscation).
- `docs/DECISION-upstream-detach.md` (détachement + watch-list cherry-pick).

### NON appliqué (raison)
- **@grpc/grpc-js** advisory (M14) : bump 1.14.4 tenté puis ANNULÉ. 1.14.4 casse
  le type-check desktop (conflit de double-copie de `Client` avec les bindings
  générés). Exploitabilité faible (socket daemon local seulement). À refaire
  proprement : regen coordonnée des bindings management-interface contre 1.14.4.
- **C2** Android `VpnService.protect()` + trous killswitch : gros + besoin
  device réel, non vérifiable ici. Top blocker, à faire en session dédiée.
- **H1** Windows pipe DACL : Windows-only, non compilable sur macOS.
- **H2** iOS `kSecAttrAccessGroup` explicite : besoin Xcode/device.
- **M10** hard-fail signer-absent : changement de comportement daemon, à
  designer (risque de casser l'état no-signer légitime).
- **P1/P3/P4/P5** (submodule, schéma relay natif, warren-client-core, split
  lib.rs) : refactos S->XL, trop gros pour push direct sur main sans review.
  Documentés ; daemon.yml Linux réécrit par l'agent CI à re-vérifier par
  dispatch manuel.

---


Audit multi-agents (7 passes parallèles : sécurité Warren, qualité Rust, architecture,
code mort/redondance, desktop Electron, mobile iOS+Android, build/CI/deps/perf).
Toutes les trouvailles sont sourcées `fichier:ligne`. Fork Mullvad, 782 commits
au-dessus de `upstream-baseline-2026-05-06`, delta 3 524 fichiers (+179k/-166k).

---

## 1. CRITIQUE

### C1. Aucune CI ne tourne automatiquement (le « CI gate » n'existe pas)
- Tous les workflows sauf `release.yml` (tag push) sont `workflow_dispatch` only
  (vérifié sur les 55 fichiers de `.github/workflows/`).
- L'hypothèse « Linux/Windows non compilables localement, la CI est le garde-fou »
  est fausse en pratique : rien ne tourne sur PR ni sur main.
- Pire : les gates dispatchables sont cassés :
  - `daemon.yml` job `build-linux` (le seul `cargo test --workspace` Linux) n'a ni
    `mullvad-build-env` ni `WARREN_CORE_RO_TOKEN` : échoue à `cargo metadata`
    (path-deps `../warren-core` introuvables).
  - `clippy.yml` job android cible `--package mullvad-jni`, crate qui n'existe plus
    (c'est `warren-jni`).
  - `rust-supply-chain.yml` appelle `mullvad-build-env` sans token : `cargo deny`
    in-runnable même à la main.
- `warren-checks.yml` (fmt + clippy + tests Warren + type-check desktop) est bien
  conçu mais dispatch-only.
- **Action** : réactiver `warren-checks.yml` + `daemon.yml` (réparé) +
  `osv-scanner-scheduled.yml` sur `push: main` / `pull_request` ; supprimer ou
  réparer les workflows upstream périmés.

### C2. Android : le tunnel ne peut PAS fonctionner sur device réel
- **Aucun `VpnService.protect()` / `Network.bindSocket` sur le chemin tunnel Warren.**
  Le seul `protect()` du repo est dans le legacy inutilisé
  (`UnderlyingConnectivityStatusResolver.kt:42`). `warren-jni` n'exporte pas de
  bypass (`bypass_cidrs` dead code, `tunnel.rs:53-54`), warren-core bind un UDP
  wildcard. Le TUN est posé avec `0.0.0.0/0`+`::/0` AVANT le dial Quinn
  (`WarrenQuinnAdapter.kt:94-100`) : les paquets du handshake sont routés dans le
  TUN lui-même (boucle de routage).
- Corollaire confirmant la non-testabilité : pas d'activity MAIN/LAUNCHER + alias
  `android:targetActivity` pointant vers une classe inexistante
  (`AndroidManifest.xml:66-82`) ; endpoint de fallback Rust
  `"warren-exit-1.warren.brown:443"` (hostname parsé en `SocketAddr` = jamais
  connectable, TLD typo) (`android_jni.rs:563`, `tunnel.rs:172`).
- **Kill switch structurellement contourné** :
  1. 15 s fail-open à chaque handover Wi-Fi↔cellulaire, même killswitch ON :
     `scheduleHandoverReconnect` annule le poll de statut, `disconnectTunnel()`
     (ferme le TUN), puis `delay(15s)` sans interface VPN
     (`WarrenQuinnAdapter.kt:379-404`).
  2. Killswitch opt-in, défaut OFF (`WarrenTunnelConfig.kt:40`) : toute mort
     inattendue du tunnel démonte le TUN et fuit (`:262-265`). Inverse le modèle
     fail-closed inconditionnel de Mullvad.
  3. Le blackhole vit avec le process : `Failed` lâche le foreground,
     `onDestroy`/`onRevoke` libèrent le trafic silencieusement.
- Multi-hop/obfuscation côté Android = mensonges UI : `entry_hop`/`obfuscation_m40`
  dead code (`tunnel.rs:44-45,72-77`) alors que l'UI les affiche actifs.
- TUN hardcodé `10.64.0.1/32` alors que l'exit alloue `assigned_ipv4` dynamique :
  pas de chemin reassign Android.
- Les fixes P0 de la mission parité (routes v6, DNS in-tunnel, blocked-TUN) sont
  réels et bien faits au niveau « plan », mais protègent un tunnel qui ne passe
  pas de trafic.
- **Verdict : non shippable, non testable sur device.** Top 3 blockers :
  (1) câbler `protect()` sur la socket UDP warren-tunnel + 1 connect réel on-device,
  (2) fermer les trous killswitch (blocking TUN avant teardown handover,
  blocked-on-error par défaut, foreground en Failed),
  (3) manifest MAIN/LAUNCHER + chemin clé non-biométrique pour le connect système.

---

## 2. HIGH

### H1. Windows : n'importe quel process local peut lire la mnémonique
- `mullvad-management-interface/src/lib.rs:270-291,336-346` : named pipe créé avec
  `allow_everyone_create()` ; `WalletAccessControl::authorize` retourne `Ok(())`
  dès que `creds: None` (toujours le cas sur pipe Windows).
- N'importe quel process local (autre user, malware non privilégié) peut appeler
  `GetWarrenMnemonic`. La mnémonique est une seed wallet non rotatable.
- C'est le reliquat du finding CRITIQUE de l'audit 2026-06-09 : **corrigé sur Unix**
  (groupe `warren`, socket 0o760, SO_PEERCRED + TOFU), **pas sur Windows**.
- **Fix** : DACL du pipe restreinte à SYSTEM + service SID + user desktop.

### H2. iOS : access group keychain wallet non épinglé (extension peut silencieusement échouer)
- `ios/Shared/WarrenWalletKeychain.swift:51-59` : item generic-password sans
  `kSecAttrAccessGroup` explicite (contrairement à
  `WarrenSettings/KeychainSettingsStore.swift:101` qui le fait).
- App et extension PacketTunnel sont des binaires distincts ; le partage repose sur
  un groupe par défaut non garanti. Si la lecture échoue, l'extension log et bail
  (`WarrenQuinnTunnelImplementation.swift:168-172`) : état « tunnel up + blackhole
  fail-closed mais jamais connecté », très dur à diagnostiquer.
- **Fix** : `kSecAttrAccessGroup` explicite + entitlement keychain-access-groups
  partagé, prouvé on-device.
- Sinon le modèle secret iOS est sain : `WhenUnlockedThisDeviceOnly`, pas de sync
  iCloud, seed 32B zéroïsée `memset_s` en deinit, rien en App Group UserDefaults.

### H3. quinn-udp double-résolu : les patchs du fork ne s'appliquent pas partout
- `[patch.crates-io]` ne couvre que `quinn` et `quinn-proto` (`Cargo.toml:187-189`).
- `cargo tree` montre DEUX `quinn-udp v0.6.1` : le forké (path, usage interne quinn)
  et celui de crates.io tiré DIRECTEMENT par `warren-tunnel v0.3.11`.
- Les patchs quinn-udp du fork (dlsym Apple fast path, GSO Windows per-socket,
  socket-buffer sizing) ne s'appliquent donc pas à l'usage direct de warren-tunnel.
- **Fix** : ajouter `quinn-udp = { path = ".../vendor/quinn-fork/quinn-udp" }` au patch.

### H4. Pin warren-core non vérifié sur les builds locaux
- `.warren-core-version` n'est lu que par la CI et `verify-beta.sh` ; `build.sh` ne
  compare jamais le HEAD local de `../warren-core` au pin.
- Incident réel déjà documenté (2026-05-29) : drift silencieux qui a shippé un build
  sans les patchs quinn GSO/obfuscation.
- **Fix** : check pin + `git describe --dirty` warren-core dans `build.sh` ; à terme
  exécuter le plan déjà écrit (`docs/warren-app-quinn-migration-plan.md`) :
  submodule `vendor/warren-core/`.

---

## 3. MEDIUM : sécurité et correctness

### Desktop (Electron)
- **M1. L'auto-clear clipboard de la mnémonique ne fire jamais** :
  `CopyMnemonicButton.tsx:42` appelle `navigator.clipboard.readText()` mais le
  permission handler n'autorise que `clipboard-sanitized-write`
  (`main/index.ts:79,1100-1107`) ; le `catch {}` avale le refus : la seed reste
  dans le presse-papier OS (et l'historique cloud) pour toujours.
  Fix : faire le clear conditionnel côté main process via IPC.
- **M2. Le gate « backup obligatoire » saute au restart de la GUI** :
  `backup-pending` n'existe qu'en mémoire renderer ; au relaunch, le device state
  rejoue `logged in` → `loggedIn` (`app.tsx:338-340,996-999`) et l'utilisateur
  atterrit sur la main view avec une identité non sauvegardée.
  Fix : persister un flag backupPending (gui-settings ou daemon).
- M3. Race create-account : un reject IPC tardif écrase `backup-pending` par
  `failed` (`app.tsx:594-599`). Guard à ajouter.
- M4. `openUrlWithAuth` colle le www-auth-token en query string des URLs checkout
  (`app.tsx:609-618`) : history/logs. Probablement dead sous le modèle wallet ;
  à vérifier puis passer en `openUrl` simple.
- M5. `savedReportId` traversal (hérité upstream) : `problem-report.ts:89-91` joint
  une string renderer dans un path ; valider UUID.
- Positif : hardening Electron intact (contextIsolation, sandbox, CSP, allowlist
  openExternal saine), mnémonique jamais dans redux/logs, devtools prod off,
  i18n 23 locales complètes (0 « Mullvad », 0 em-dash), bindings gRPC frais
  (108/108 RPCs, note mémoire « stale bindings » obsolète).

### Daemon / Rust
- **M6. `Utc::now().timestamp() as u64`** wrap sur horloge pré-epoch →
  `time_added` énorme (`device/account_backend.rs:183`).
  Fix : `u64::try_from(...).unwrap_or(0)`.
- **M7. `lock().ok()` sur `pump_error_tx`** avale l'escalade si mutex poisoned :
  tunnel affiché « Connected » alors que mort
  (`talpid-warren-tunnel/src/lib.rs:1734,1826`, `migration_watchdog.rs:498`).
  Fix : `.unwrap_or_else(|p| p.into_inner())`.
- M8. Commentaire ABA trompeur dans `migration_watchdog.rs:134` (la logique `!=`
  est correcte mais la justification écrite est fausse ; risque de « fix » futur
  qui casse). Corriger le commentaire + test distinguant les cas.
- M9. `is_plaintext()` jamais appelé en prod : aucun warning quand la mnémonique
  tombe en fallback plaintext (`os_secret_storage/mod.rs:94`). Logger + surfacer.
- M10. `*_or_signed` (rest.rs:743-856) : downgrade silencieux en Bearer non signé
  quand le signer est absent ; le daemon devrait hard-fail visiblement
  (ErrorState AuthFailed) plutôt que boucler en 401.
- M11. Footguns TOFU : `load_from_cache_dir` public sans pin (selector) ;
  mode TOFU wallet world-accessible quand le groupe `warren` manque (race au boot).
  Fix : API pin obligatoire ; gater le TOFU wallet hors release.
- M12. `unwrap_or_else(|_| reqwest::Client::new())` perd timeouts/config
  (`warren_multi_hop_directory.rs:509`, `warren_relay_list_updater.rs:76`) →
  `expect`.
- **M13. Custom lists silencieusement dégradées en `Any`**
  (`warren_query_from_settings.rs:45-48`) : bug UX réel, un utilisateur qui choisit
  une liste custom obtient « n'importe quel exit » sans avertissement.
  Wirer ou masquer la feature.

### Supply chain / build
- M14. Scan sécurité éteint : osv-scanner jamais exécuté, ignore RUSTSEC-2025-0141
  expiré le 2026-06-07 (`osv-scanner.toml:74-76`), pas de dependabot/renovate,
  advisory high `@grpc/grpc-js 1.14.0-1.14.3` fixable en 1 ligne côté desktop.
- M15. **Data plane compilé en `opt-level = "s"`** : la liste d'overrides
  `[profile.release.package]` (quinn/ring à opt 3) n'a jamais été étendue aux
  crates qui bougent les paquets maintenant : `warren-tunnel`, `warren-client`,
  `warren-protocol`, `warren-multihop`, chacha20poly1305/hpke. Gains single-digit %
  plausibles, gratuit.
- M16. Prod Linux ship `-vv` (Debug) + tick `pump_metrics` debug 2 s toujours actif
  (`talpid-warren-tunnel/src/lib.rs:1216-1240`) ≈ 43k lignes/jour/client.
  Passer Info + métriques à 30 s ou trace.

---

## 4. Architecture : verdict et refactos proposés

### Constat central : le tracking upstream est économiquement mort
Delta = la majorité du produit (Android réécrit sans daemon, iOS renommé en masse,
daemon `lib.rs` 3 643 → 4 923 lignes, proto +15 RPCs). Un rebase upstream coûterait
plusieurs personne-semaines par release, récurrent, pour du code que Warren a
supprimé (WireGuard, API Mullvad, obfuscation).
**Recommandation : détacher officiellement**, garder le remote upstream pour du
cherry-pick chirurgical sur les couches plate-forme peu modifiées
(talpid-routing, talpid-dns, talpid-net, firewall, mullvad-leak-checker,
split-tunneling).

### Risque n°2 : la logique client existe en 3 exemplaires à 3 maturités
Desktop (state machine complète + multihop + TOFU + NAT-PMP), iOS (2e impl
indépendante de la vérification directory multihop : `warren-ios/src/warren_multihop_directory.rs`
vs `mullvad-daemon/src/warren_multi_hop_directory.rs`), Android (3e orchestration
Kotlin sans multihop ni TOFU). Ce sont exactement les invariants de sécurité
(signature directory, anti-rollback, pinning, killswitch, doctrine retry) qui sont
dupliqués.

### Wart desktop : le shim relay-list fake-WireGuard
`warren_relay_list_view.rs` fabrique une RelayList Mullvad avec fausses clés x25519,
hostnames slugifiés et table de centroïdes pays hardcodée ; impose le maintien de
`talpid-types::net::wireguard` + `TunnelType::WireGuard` (encore `#[default]` !) et
plafonne le scaling multi-régions (carte fausse par construction, grammaire selector
limitée à Any|Country|City).

### Refactos recommandés (ordre)
1. **P1 (S)** : submodule `vendor/warren-core/` + check de pin local + façade
   d'import (seuls `talpid-warren-tunnel` et une petite façade importent
   warren-core ; le daemon importe la façade). Tue le mode d'échec
   « mauvais binaire shippé » déjà survenu.
2. **P2 (M)** : acter le détachement (remplacer l'intention périmée de
   `UPSTREAM_BASELINE.md`) + purge du poids mort : `mullvad-masque-proxy/`
   (orphelin, ~2 560 LOC, hors workspace), migrations settings v1-v12 (aucun parc
   installé Mullvad), stack access-method/shadowsocks/encrypted-dns dans
   mullvad-api+daemon (surface d'attaque inerte dans le chemin HTTP le plus
   sensible), `RelaySelector`/`RelayListUpdater` legacy (dead feeding dead),
   iOS `MullvadPostQuantum` + scripts obfuscator.
3. **P3 (M/L)** : schéma relay Warren natif end-to-end (coordonnées dans
   `/v1/exits`, message proto natif, renderer) ; retirer le shim fake-WG, la table
   centroïdes, puis `TunnelType::WireGuard`. Étendre la grammaire selector
   (custom lists, hostname) au passage. Prérequis au scaling multi-régions.
4. **P4 (L/XL)** : crate `warren-client-core` (dans warren-core) qui possède
   directory fetch+verify+anti-rollback, sélection exit, TOFU pinning, doctrine
   retry, supervision session, exposée via FFI (UniFFI) ; `warren-jni` et
   `warren-ios` deviennent des bindings minces, le daemon consomme le même cœur.
   Commencer par unifier les 2 impls multihop-directory (pire risque de divergence
   sécurité), puis amener Android à parité PAR le core.
5. **P5 (S)** : éclater `mullvad-daemon/src/lib.rs` (4 923 l.) et
   `talpid-warren-tunnel/src/lib.rs` (4 036 l.) en modules
   (params/monitor/dispatch/pump ; arbre `warren/` côté daemon). Mécanique, risque
   quasi nul, rend le delta lisible.

---

## 5. Code mort / nettoyages (priorisés)

1. Supprimer `mullvad-masque-proxy/` (zéro référence, non buildé) + la ligne stale
   dans `warren-ios/deny.toml:1`.
2. Fix custom-list fallback (cf. M13).
3. Décision produit vouchers : flow desktop complet (~800 LOC route+view+IPC) câblé
   vers un backend qui n'existe pas (billing = Stripe checkout). Si out : retirer +
   stubber `SubmitVoucher` comme les RPCs WG.
4. Rebrand CLI : 7 « Mullvad » dans `mullvad-cli/src/main.rs` help text ; vérifier
   les chemins logs de `mullvad-problem-report/src/lib.rs:281-303` (« Mullvad
   VPN/logs ») vs les chemins Warren réels, sinon les rapports de problème ne
   collectent pas les bons logs.
5. Retirer/feature-gater les sous-commandes CLI mortes (`anti-censorship`,
   `tunnel mtu/quantum-resistant/daita/allowed-ips`) : no-ops qui sortent en succès.
6. Deps inutilisées (cargo-machete, à confirmer par build) : chrono+ipnetwork
   (mullvad-api), talpid-platform-metadata (daemon), warren-config+warren-relay
   (talpid-warren-tunnel), hex (warren-ios), etc.
7. Traduire les blocs de commentaires français dans
   `management_interface.proto:59-87` (violation règle English-only) + corriger le
   commentaire périmé SetWarrenMnemonic (« restart manuel » alors que hot-swap).
8. Conventions CLAUDE.md côté Rust : commentaires narration « Step 1/2/3 »
   (`warren_signer.rs:259-271`), tombstones « M-1 fix: ... » (lib.rs:1083,1705),
   ~15 références « Session H.6 / M5.B.1 » dans des doc comments publics,
   box-drawing `─` (U+2500) comme dividers.
9. À GARDER (rebase/compat malgré le détachement partiel) : variantes
   `TunnelType::WireGuard` tant que le shim P3 vit, champs settings WG
   (désérialisation), stubs RPC WG, migrations v6-v13 si un parc beta existe.

## 6. Corrections de notes mémoire (état réel vérifié)
- « stale desktop grpc dist bindings » : OBSOLETE, bindings frais (108/108 RPCs,
  regen c759cb1946 postérieure au dernier changement proto).
- « 22 langs English-fallback » : OBSOLETE, 23 locales traduites, 0 Mullvad.
- « 434 Mullvad strings iOS » : OBSOLETE, 0 dans tous les .xcstrings.
- « lockdown reset on shutdown » : FIXÉ (`lib.rs:4548-4554` + persistance Windows
  upgrade `:4563-4587`).
- « mnemonic via world-readable mgmt socket » : fixé Unix, OUVERT Windows (H1).
- « Linux v4 route-split 2 impls » : ne s'applique PAS à warren-app (façade unique
  `talpid-warren-tunnel/src/default_route_split.rs`) ; la redondance vit dans
  warren-core, à tracker là-bas.

## 7. Points sains (à ne pas « réparer »)
- Intégration state machine talpid propre (`BackendParams` enum, tunnel_monitor
  127 l., pas de special-cases bolted-on).
- Crates tunnel legacy déjà supprimées proprement (talpid-wireguard,
  wireguard-go-rs, tunnel-obfuscation, talpid-tunnel-config-client : hors workspace).
- iOS data plane réellement câblé (multihop supervisor + pumps réels), fail-closed
  par construction (aucun `cancelTunnelWithError`, settings blackhole posés avant
  connect), blackhole IPv6 correct, DNS forcé in-tunnel (`matchDomains [""]`).
- Fix DNS macOS 67c4b5ea6b correct et testé (politique « never downgrade a
  captured Some »).
- Pas de sync-mutex-across-await ; boucles courtes bornées ; release pipeline
  dé-Mullvad-ifié (secrets Warren, signing opt-in).
