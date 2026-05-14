# Warren-App Quinn Migration Plan

> Plan d'exécution pour migrer warren-app du stack Iroh+noq (POC) vers
> Quinn upstream + warren-tunnel post-migration côté warren-core
> (achevée mai 2026, cf. `../warren-core/docs/17-QUINN-MIGRATION-NOTES.md`).
> Branche cible : `migration/quinn-app` à partir de `warren-base`.
> Statut : Phase 1 audit terminée, Phase 2 exécution démarre.

---

## Audit summary

Stack actuel warren-app pointe sur la version Iroh 0.98.2 via `iroh` direct
+ noq fork (`noq` / `noq-proto` / `noq-udp`) déclaré dans
`[patch.crates-io]`. La dépendance `warren-iroh-tunnel` (path-dep cross-repo
`../../warren-core/crates/warren-iroh-tunnel`) ne résout plus : warren-core a
renommé le crate en `warren-tunnel` (étape 7 de la migration upstream).

Le périmètre concret du chantier est plus étroit que la liste de fichiers
suggère :

| Fichier | Effort | Catégorie |
|---|---|---|
| `talpid-warren-iroh/Cargo.toml` | 5 LoC | déps |
| `talpid-warren-iroh/src/lib.rs` | ~50 sites | imports + types iroh→Warren |
| `talpid-warren-iroh/src/adapter.rs` | 1 site | import `warren_iroh_tunnel` |
| `talpid-warren-iroh/src/default_route_split.rs` | 0 | pas de symbole iroh |
| `mullvad-daemon/src/warren_iroh_params.rs` | 0-2 sites | param struct, types via talpid-warren-iroh |
| `mullvad-daemon/src/warren_signer.rs` | 0 | n'utilise pas iroh (déjà via warren-identity) |
| `mullvad-daemon/src/warren_relay_list_view.rs` | 0 | n'utilise pas iroh dans le code source (tests seulement) |
| `mullvad-daemon/src/tunnel.rs` | ~3 sites | re-export `WarrenIrohParameters` |
| `mullvad-daemon/src/lib.rs` | ~3 sites | idem |
| `mullvad-daemon/src/device/device_backend.rs` | 0 | n'utilise pas iroh directement |
| `talpid-core/src/tunnel_state_machine/backend_params.rs` | tests + 1 import | `iroh::*` dans tests, `WarrenIrohParameters` ailleurs |
| `talpid-core/src/tunnel_state_machine/connecting_state.rs` | 1 import | re-export |
| `talpid-core/src/tunnel_state_machine/tunnel_monitor.rs` | 1 import | re-export |
| `talpid-core/src/tunnel_state_machine/mod.rs` | 1 site | re-export |
| `talpid-core/src/firewall/linux.rs` | 0 | invariants firewall valides post-quinn |
| `mullvad-api/Cargo.toml`, `Cross.toml` | 1-2 lignes | déps |
| `Cargo.toml` workspace | swap `[patch.crates-io]` noq→quinn | déps |

**Total : ~80 sites de modifications + workspace patch swap**. 80 % mécanique
(regex find/replace + type swap), 20 % structurel (filter_endpoint_addr,
detect_default_local_ip qui utilisaient `iroh::TransportAddr`).

**Effort : 4-6 heures pour la migration code + 1-2 h pour les tests et la
validation cargo workspace + 1 h pour le rapport.**

---

## Stratégie de partage warren-core ↔ warren-app recommandée

### Options évaluées

| Option | Pro | Con | Effort transition |
|---|---|---|---|
| **A** git submodule warren-core sous `vendor/warren-core/` | reproductible CI, version pin explicite, licence MIT/Apache préservée | friction dev local (`git submodule update --remote`) | ~2 h |
| **B** registry privé Cargo (Gitea/Forgejo) | architecture propre, séparation nette, version explicite | infra à monter (Forgejo cargo registry beta), workflow CI publish | 1-2 jours |
| **C** path-dep dev + vendor copies CI | hybride | divergence lock probable | ~1 jour |
| **D** statu quo path-dep cross-repo `../../warren-core/` + CI dual-clone à SHA pinned | aucun changement structurel | fragile, pas de version pin natif | ~30 min CI |
| **E** mono-repo | une seule source | contamination GPL→MIT, rejetée | N/A |

### Recommandation

**Court terme (cette mission, ne pas bloquer le merge migration)** : **D améliorée**.

- Garder le path-dep `../../warren-core/crates/warren-tunnel`.
- Ajouter un fichier `.warren-core-version` à la racine warren-app contenant la
  SHA git de warren-core compatible.
- Documenter dans `docs/warren-fork.md` le check-out attendu :
  `git clone warren-core ../warren-core && cd ../warren-core && git checkout <sha>`.
- CI : script `scripts/setup-warren-core.sh` qui clone à la bonne SHA si absent.

**Moyen terme (M5+)** : passer à **A git submodule** une fois warren-app stabilisé
post-migration. Bénéfice : un seul `git clone --recursive` reproduit l'env.

**Long terme (Phase 2 produit, revenue-funded)** : transition vers **B registry
Forgejo** pour découplage propre, alignement avec la doctrine "deux repos
distincts" (warren-core MIT, warren-app GPL).

### Action immédiate Phase 2

- **Phase 2.1** : ajouter `.warren-core-version` (= SHA `main` warren-core current)
  + ajouter dans `Cargo.toml` workspace warren-app :
  ```toml
  [patch.crates-io]
  quinn = { path = "../warren-core/vendor/quinn-fork/quinn" }
  quinn-proto = { path = "../warren-core/vendor/quinn-fork/quinn-proto" }
  ```
  (Remplace les patches noq/noq-proto/noq-udp obsolètes.)

---

## Code à extraire / consolider

L'audit révèle 0 duplication réelle de logique métier entre warren-core et
warren-app :

| Module candidat | Verdict |
|---|---|
| `mullvad-daemon/src/warren_signer.rs` vs `warren-identity` | **Pas de duplication**. warren_signer consomme déjà `warren_identity::{seed_from_mnemonic, derive_node_key, load_or_create_mnemonic}`. Couches orthogonales (orchestration disque vs crypto pure). |
| `mullvad-daemon/src/warren_iroh_params.rs` vs `warren-protocol::Setup` | **Abstractions orthogonales**. Le params struct daemon-side se traduit en `Setup` au handshake côté warren-tunnel. Pas de redondance. |
| `mullvad-daemon/src/warren_relay_list_view.rs` vs `warren-relay-selector::JsonRelay` | **Cosmétique seulement**. Le mapping `WarrenRelay` → `WireguardRelay` est une "fake view" pour préserver la GUI Mullvad. Refacto cosmétique non bloquant. |
| `talpid-warren-iroh/src/adapter.rs` vs `warren-tunnel::PacketDevice` | **Pas duplication**. L'adapter wrappe `tun08::AsyncDevice` Mullvad (crate `tun 0.8.5`) pour le contrat `PacketDevice` Warren (crate `tun-rs 2`). Le bridge cross-crate `tun08 ≠ tun-rs` reste nécessaire post-quinn. Pas d'upstream possible sans aligner les versions de la crate `tun` côté warren-core (= chantier séparé hors scope migration). |

**Conclusion** : aucun code à déplacer warren-app → warren-core (license GPL→MIT
serait illégale anyway). Aucun déplacement warren-core → warren-app à faire.
Migration purement orientée déps + type swap.

---

## Plan migration code en sous-étapes (TDD)

Branche : `migration/quinn-app` à partir de `warren-base`.

### 2.1 Préparation déps

- Créer branche `migration/quinn-app`.
- Ajouter `.warren-core-version` (SHA `main` warren-core).
- `Cargo.toml` workspace warren-app : swap `[patch.crates-io] noq/noq-proto/noq-udp` → `quinn/quinn-proto` pointant vers `../warren-core/vendor/quinn-fork/`.
- `talpid-warren-iroh/Cargo.toml` :
  - swap `warren-iroh-tunnel = { path = ... }` → `warren-tunnel + warren-protocol + warren-tls`.
  - swap `iroh = "=0.98.2"` → `quinn = "0.11"` + `ed25519-dalek` reste.
- `cargo check -p talpid-warren-iroh` : doit échouer (types iroh manquants). RED OK.

### 2.2 Port talpid-warren-iroh/src/lib.rs (gros bloc)

Sites à modifier :

| Avant | Après |
|---|---|
| `use iroh::{EndpointAddr, EndpointId}` | `use warren_protocol::{WarrenExitAddr, WarrenPubkey}` |
| `use warren_iroh_tunnel::{...}` | `use warren_tunnel::{...}` |
| `params.exit_id: EndpointId` | `params.exit_id: WarrenPubkey` (peut être déplacé dans `WarrenExitAddr.id`) |
| `params.exit_addr: EndpointAddr` | `params.exit_addr: WarrenExitAddr` |
| `client.connect(exit_id, exit_addr)` | `client.connect(exit_addr)` (signature unifiée post-migration warren-core) |
| `client.connect_multi(exit_id, exit_addr, n)` | `client.connect_multi(exit_addr, n)` |
| `EndpointAddr::new(id)` | `WarrenExitAddr::new(id)` (id devient `WarrenPubkey`) |
| `EndpointAddr::with_ip_addr(addr)` | `WarrenExitAddr::with_ip_addr(addr)` |
| `addr.ip_addrs()` | `addr.ip_addrs()` (API préservée) |
| `addr.id` | `addr.id` (préservé, juste type différent) |
| `iroh::TransportAddr::Ip(socket)` | `WarrenTransportAddr::Ip(socket)` |
| `iroh::SecretKey::from_bytes(...).public()` | `ed25519_dalek::SigningKey::from_bytes(...).verifying_key()` → `WarrenPubkey::from_bytes(vk.to_bytes())` |
| `EndpointId::from_bytes(&[u8; 32])` | `WarrenPubkey::from_bytes([u8; 32])` (no Result, const) |
| Test `0001000100` substring check | Adapter au nouveau hex format `WarrenPubkey::to_hex()` |

**API publique du crate `talpid-warren-iroh`** : `WarrenIrohParameters` change ses types de champ (`EndpointId → WarrenPubkey`, `EndpointAddr → WarrenExitAddr`). Breaking change pour les consommateurs (mullvad-daemon, talpid-core).

Tests touchés (en bloc) :
- `warren_iroh_parameters_debug_does_not_leak_secrets`
- `filter_endpoint_addr_*`
- `is_routable_internet_*` : inchangé, c'est juste de l'IP filtering.

### 2.3 Port talpid-warren-iroh/src/adapter.rs

Un seul site : `use warren_iroh_tunnel::PacketDevice` → `use warren_tunnel::PacketDevice`.

### 2.4 Port mullvad-daemon

`tunnel.rs`, `lib.rs`, `warren_iroh_params.rs` : `use talpid_warren_iroh::WarrenIrohParameters` reste valide (rename éventuel du crate plus tard). Les types des champs internes ont changé mais l'API publique du crate `talpid-warren-iroh` reste.

`device/device_backend.rs` : si utilise `ed25519_dalek::SigningKey` (déjà OK).

### 2.5 Port talpid-core

`tunnel_state_machine/{backend_params,connecting_state,tunnel_monitor,mod}.rs` :
- Imports `talpid_warren_iroh::{WarrenIrohMonitor, WarrenIrohParameters}` restent valides.
- Tests dans `backend_params.rs` qui font `use iroh::{EndpointAddr, SecretKey}` ; swap vers `warren_relay_selector::warren_types::{WarrenExitAddr, SigningKey}` ou imports directs `warren_protocol`.

`firewall/linux.rs` : aucune action. Les invariants `skuid == ROOT_UID` + cgroup split_tunnel + fwmark restent valides : le bug `n0_nat_traversal` est structurellement éliminé (warren-tunnel ne fait plus de path discovery côté Quinn upstream), donc plus de chemin `tun0 → encap → tunnel → ...` parasite.

### 2.6 Rename optionnel `talpid-warren-iroh` → `talpid-warren-tunnel`

À évaluer en fin de migration. Effort : ~10 min `git mv` + find/replace.
Bénéfice : cohérence avec warren-core (`warren-iroh-tunnel` → `warren-tunnel`).
Risque : touche ~15 fichiers (Cargo.toml workspace + imports).

**Recommandation** : oui mais en commit séparé final, après que tout compile.

### 2.7 Validation

```bash
cargo check --workspace
cargo build --release -p mullvad-daemon
cargo build --release -p mullvad-cli
cargo test -p talpid-warren-iroh -p talpid-core
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

Pas de bench Hetzner (mission code-only). Bench fork E2E Linux à programmer
ultérieurement via `bench/scripts/fork-e2e-linux.sh` côté warren-core.

---

## Risques identifiés

1. **Patch GSO quinn-fork non propagé** : warren-app workspace doit redéclarer
   `[patch.crates-io] quinn = { path = "../warren-core/vendor/quinn-fork/quinn" }`
   sinon Cargo résout l'upstream Quinn sans le patch ETHZ → perte de 25 %
   downlink throughput. **Mitigation** : ajout du patch + vérif via
   `cargo tree -p talpid-warren-iroh | grep quinn` qui doit montrer le path
   local.

2. **Iroh deep dependency tree** : `iroh 0.98.2` tirait pkarr, DHT, relays.
   Au cargo check, des transitives peuvent persister via d'autres crates.
   **Mitigation** : `cargo tree -p mullvad-daemon | grep -E "iroh|noq"` doit
   être vide après migration.

3. **Régression WireGuard kernel** : la migration ne touche PAS `talpid-wireguard`
   ni les variants `Wireguard` du `TunnelBackend`/`BackendParams`. Le dispatch
   `warren_mode` continue de router vers WG si désactivé. **Mitigation** :
   tester en local `mullvad-daemon` sans flag warren → tunnel WG opérationnel.

4. **Build cross-platform** : warren-app supporte Linux/macOS/Windows/Android/iOS.
   Quinn upstream est cross-platform mais les attributs `#[cfg(target_os = ...)]`
   dans `talpid-warren-iroh/src/lib.rs` (routing split-default Linux+macOS,
   pas Windows) restent valides. **Mitigation** : ne pas tester Android/iOS
   dans cette session (pas d'env), commit avec note `pending mobile validation`.

5. **GUI Electron IPC** : `mullvad-management-interface/proto/*.proto` ne
   référence pas Iroh/Quinn directement. Aucun impact attendu.

6. **Tests warren-relay-selector côté warren-core** : 4 tests dans
   `crates/warren-relay-selector/tests/` (selection, weighted, retry_attempt,
   relay_types portés mais 3 autres cassés) référencent `iroh_types::` qui
   n'existe plus. **Out of scope cette mission** (bug warren-core). À noter en
   follow-up.

---

## Estimation effort jour-ingé

| Phase | Estimation |
|---|---|
| Phase 1 audit | DONE (~1 h) |
| Phase 2.1 préparation déps | 30 min |
| Phase 2.2 port lib.rs | 2 h |
| Phase 2.3 port adapter.rs | 5 min |
| Phase 2.4 port mullvad-daemon | 30 min |
| Phase 2.5 port talpid-core | 30 min |
| Phase 2.6 rename optionnel | 15 min |
| Phase 2.7 validation cargo | 1 h (compile + fix résiduels) |
| Phase 3 rapport | 30 min |
| **Total mission** | **~6 h** (= 1 jour-ingé avec marge) |

Suivi du bench fork E2E Linux à programmer ultérieurement (1 jour
supplémentaire, hors scope).
