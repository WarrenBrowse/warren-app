# Phase M4.H.B - Câblage stack M4.E.D dans talpid-warren-tunnel

> Brief d'agent autonome cross-repo warren-core (lecture+tests) +
> warren-app (dev gros chunk + bench). Doctrine NOUVELLE §0.5 full
> autonomy NO timid rollback (cf. memory
> `feedback_agent_full_autonomy_no_timid_rollback`).
> La commande `/goal` compagne pointe vers ce fichier.

**Effort estimé** : wall-clock 3-5 jours.
**Coût Hetzner** : ≤ 0.30 EUR (bench multi-hop nécessite 3 nodes
nbg1 + fra1 + hel1 ≈ 0.04 EUR/h × ~6h cumulés runs).
**Pré-condition** :
- warren-app main HEAD post-M4.H.A.quart (33e3adbe0a ou descendant)
- warren-core main HEAD `8f4f299+` (fix allowlist apply_snapshot)
- warren-exit-1 prod hel1 sur HEAD warren-core+fix (validé M4.H.A.quart)
- warren-backend-api v0.2.11 prod (intact)

**Objectif** : étendre `talpid-warren-tunnel` pour câbler la stack
complète M4.E.D depuis warren-core. Ajouter path-deps `warren-multihop`,
`warren-client`, `warren-relay`, `warren-backoff`. Étendre
`WarrenTunnelParameters` côté daemon Mullvad pour porter les params
multi-hop (RelayDescriptorSigned + ExitDescriptorSigned + toggle).
Dispatcher single-hop vs multi-hop selon settings. Brancher
`MultiHopSupervisor` au tunnel state machine `talpid-core` pour
auto-reconnect transparent. Tests régression single-hop + multi-hop.
Bench cross-DC empirique des deux modes.

---

## 0. MANDAT STRICT

Anti-patterns historiques M4.E §7 interdits. TDD strict warren-core
CLAUDE.md §1 (RED → GREEN → REFACTOR) sur tout nouveau code Rust
côté warren-app. /v1 constantes IMMUABLES (escalade pour /v2).

---

## 0.5 MANDAT D'AUTONOMIE (cf. memory `feedback_agent_full_autonomy_no_timid_rollback`)

**Tu as plein mandat pour atteindre le verdict GO.** Si une étape fail
ou imprévu apparaît :
- Diagnostic 30 min max
- Fix tactique TDD strict (cross-repo warren-core + warren-app OK)
- Commit + push origin/main du repo concerné
- Reprise procédure avec fix appliqué

**PAS de rollback opportuniste. PAS d'escalade "pour valider l'approche".
TU DÉCIDES, TU COMMIT, TU PUSH.**

Escalade `AskUserQuestion` SEULEMENT si :
1. Secret leak découvert
2. Coût Hetzner > 0.30 EUR
3. Breaking change /v1 wire format (ALPN, HKDF, AAD, descriptor schema)
4. Signing key prod doit être touchée

Verdict NO-GO seulement si après 4h investigation+fix tentés, le
problème dépasse l'agent (probablement archi nécessite refactor large).

Décisions tactiques que tu peux prendre seul (exemples) :
- Naming des nouveaux fields `WarrenTunnelParameters` (multi_hop_enabled
  vs multihop ou autre)
- Choix dispatch single/multi via enum tagged dans params OU bool +
  Option<MultiHopConfig>
- Architecture du wiring MultiHopSupervisor → talpid-core state machine
  (callback vs channel vs Arc<Mutex>)
- Order des sub-tasks si tu vois une meilleure séquence

---

## 1. Optimisations agent

- Lectures sources cross-repo en PARALLÈLE en début de phase
  (warren-multihop + warren-client + warren-relay + warren-backoff
  + 5-10 fichiers daemon Warren existant en même temps)
- Tests warren-app `cargo test -p talpid-warren-tunnel` groupés en
  fin de sous-tâche, pas entre chaque edit
- Cargo workspace check unique en fin de phase
- Cross-compile Linux x86_64 unique en fin de phase pour bench
- scp parallèle vers 3 nodes Hetzner (client + relay + exit)

---

## 2. Setup initial

```bash
cd /Users/poka/dev/warrenBros/warren-app
git status                                  # clean main HEAD post-quart
git log --oneline -3
cd /Users/poka/dev/warrenBros/warren-core
git status                                  # clean
git log --oneline -1                        # MUST be 8f4f299+ (fix allowlist)
cat /Users/poka/dev/warrenBros/warren-app/.warren-core-version
                                            # MUST be 8f4f299+ (bump si pas le cas)
export WARREN_SSH_KEY=pokash
export HCLOUD_CONTEXT=warren
```

Si pin obsolète : bumper `.warren-core-version` côté warren-app dès
M4.H.B.0, commit séparé `chore(warren-core-pin): bump to <sha> for
allowlist fix integration`.

---

## 3. Sources à lire (PARALLÈLE - un seul message multi-Read)

### Warren-core crates à intégrer

- `crates/warren-multihop/src/lib.rs` (exports principaux)
- `crates/warren-multihop/src/session.rs` (ClientSession, ExitSession,
  HPKE wire format /v1 figé, direction-tagged)
- `crates/warren-multihop/src/protocol.rs` (frames, descriptors)
- `crates/warren-client/src/lib.rs` (build_tuned_client,
  with_bind_local_ip, ClientTunnel single-hop)
- `crates/warren-client/src/multi_hop.rs` (MultiHopClient,
  WarrenPumpHandle, MultiHopSupervisor M4.E.D)
- `crates/warren-relay/src/lib.rs` (PKI, ExitConnPool, forward_session,
  RelayDescriptorSigned /v1)
- `crates/warren-backoff/src/lib.rs` (Backoff générique, HANDSHAKE
  constant)
- `crates/warren-protocol/src/lib.rs` (déjà connu : WarrenPubkey,
  WarrenExitAddr, descriptors signed /v1)
- `crates/warren-config/src/lib.rs` (constantes /v1 : ALPN_H3,
  SUFFIX_V2, HKDF salt/info, TUNNEL_INITIAL_MTU=1280)

### Warren-app daemon-side existant

- `mullvad-daemon/src/warren_tunnel_params.rs` (WarrenTunnelParameters
  actuel, à étendre)
- `mullvad-daemon/src/warren_relay_selector.rs` (selection relay+exit,
  à adapter pour multi-hop dispatch)
- `mullvad-daemon/src/warren_query_from_settings.rs` (query depuis
  settings persisté)
- `mullvad-daemon/src/warren_relay_list_view.rs` (vue GUI relays)
- `mullvad-daemon/src/warren_mode.rs` (toggle warren_mode général)
- `mullvad-daemon/src/warren_remote_config.rs` (config remote)
- `mullvad-daemon/src/warren_signer.rs` (load/derive Ed25519 + voucher
  flows, lié à caveat factory LOCAL=0)
- `mullvad-daemon/src/warren_device_bootstrap.rs` (bootstrap device
  identity)
- `talpid-warren-tunnel/src/lib.rs` (entry point adapter à refactor
  pour dispatch single/multi)
- `talpid-warren-tunnel/Cargo.toml` (path-deps à étendre)
- `talpid-core/src/tunnel_state_machine/mod.rs` (state machine entry)
- `talpid-core/src/tunnel_state_machine/connecting_state.rs`
- `talpid-core/src/tunnel_state_machine/connected_state.rs`
- `talpid-core/src/tunnel_state_machine/backend_params.rs`
  (BackendParams enum, variant Warren à étendre)

### Memory cross-session

- `warren_multihop_doctrine_v1.md` warren-core (architecture HPKE +
  two-relayed QUIC pattern Apple Private Relay, toggle OFF default,
  PMTU 1280 figé /v1, ALPN h3, SUFFIX_V2 `.exits.warrenbrowse.com`,
  HPKE epoch < 8h rotation)
- `warren_m4e_delivered.md` warren-core (récap toutes phases M4.E
  incluant M4.E.D auto-reconnect MultiHopSupervisor)
- `warren_obfuscation_doctrine_v1.md` warren-core (M4.0 invariants
  toujours actifs, TUNNEL_INITIAL_MTU = 1280, warren_first_initial_
  crypto_chunk = Some(64))
- `feedback_warren_phase_prompts_no_branch.md` warren-core
- `feedback_warren_agent_optimization.md` warren-core
- `feedback_agent_full_autonomy_no_timid_rollback.md` warren-app
  (CRITIQUE - lue avant le brief)
- `project_warren_app_state_post_quinn.md` warren-app
- `warren_m4h_a_quart_delivered.md` warren-app (état post-quart prod
  M4.H.A baseline 802 Mbps single-hop validé)

---

## 4. Plan d'exécution

### M4.H.B.0 - Pin bump + path-deps extension

1. Si `.warren-core-version` < `8f4f299` : bumper. Commit séparé
   `chore(warren-core-pin)`.
2. Ajouter path-deps dans `talpid-warren-tunnel/Cargo.toml` :
   ```toml
   warren-multihop = { path = "../../warren-core/crates/warren-multihop" }
   warren-client = { path = "../../warren-core/crates/warren-client" }
   warren-relay = { path = "../../warren-core/crates/warren-relay" }
   warren-backoff = { path = "../../warren-core/crates/warren-backoff" }
   warren-config = { path = "../../warren-core/crates/warren-config" }
   ```
3. `cargo check -p talpid-warren-tunnel` : doit passer (path-deps
   résolvent vers warren-core HEAD).
4. Commit `chore(talpid-warren-tunnel): add warren-multihop/client/relay/backoff/config path-deps`.

### M4.H.B.1 - Étendre WarrenTunnelParameters

1. Lire `warren_tunnel_params.rs` actuel pour comprendre la structure.
2. Étendre `WarrenTunnelParameters` côté daemon :
   ```rust
   pub struct WarrenTunnelParameters {
       // existing single-hop fields preserved
       pub exit_addr: WarrenExitAddr,
       pub signing_key: SigningKey,
       // ...

       // NEW for multi-hop
       pub multi_hop: Option<MultiHopConfig>,
   }

   pub struct MultiHopConfig {
       pub relay_descriptor: RelayDescriptorSigned,  // /v1 figé
       pub exit_descriptor: ExitDescriptorSigned,    // /v1 figé
       pub hpke_epoch_rotation: Duration,            // default 4h, max 8h
   }
   ```
3. Default = single-hop (`multi_hop: None`).
4. TDD : test RED qui assert `WarrenTunnelParameters::default()` single-
   hop, test qui parse settings multi_hop=true et hydrate
   `MultiHopConfig`.
5. Update `warren_query_from_settings.rs` pour hydrater
   `multi_hop: Option<MultiHopConfig>` depuis settings persisté.

### M4.H.B.2 - Adapter warren_relay_selector pour dispatch multi-hop

1. Lire `warren_relay_selector.rs` actuel (16.5K, complexe).
2. Étendre logique sélection : si `multi_hop_enabled` setting → choisir
   un relay (1+) ET un exit (1+) depuis la relay list, sinon single-hop
   habituel.
3. La sélection doit retourner les descriptors signés (
   RelayDescriptorSigned + ExitDescriptorSigned /v1 figés) lookés up
   depuis `warren_relays_fetch.rs` (qui poll warren-backend-api).
4. TDD : tests selection single, selection multi (1 relay + 1 exit),
   selection multi avec exit_country filter, fallback si relay pool
   vide → single-hop avec warning log.

### M4.H.B.3 - Refactor talpid-warren-tunnel dispatcher

1. Lire `talpid-warren-tunnel/src/lib.rs` actuel pour identifier le
   point d'entrée `start_tunnel` ou équivalent.
2. Implémenter dispatcher :
   ```rust
   pub async fn start_tunnel(params: WarrenTunnelParameters, ...) {
       match params.multi_hop {
           None => start_single_hop(params, ...).await,
           Some(cfg) => start_multi_hop(params, cfg, ...).await,
       }
   }
   ```
3. `start_single_hop` = existing path via `warren_client::ClientTunnel`
   + `with_bind_local_ip` + tuned client (M4.H.A path validé 802 Mbps).
4. `start_multi_hop` = new path via `warren_client::MultiHopClient` :
   - Open C1 vers relay (descriptor signed)
   - Forward HPKE-encrypted payloads dans Quinn datagrams C1
   - Relay terminate C1, open C2 vers exit, forward datagrams C2
   - Exit terminate C2, déchiffre HPKE (X25519 privkey exit), SNAT
   - Reverse exit→client via direction-tagged HPKE (`0x02` byte trailing,
     cf. `warren_multihop_doctrine_v1.md`)
5. TDD : test single-hop dispatch (mock backend), test multi-hop dispatch
   (mock relay+exit), test fallback graceful sur erreur HPKE setup.

### M4.H.B.4 - Brancher MultiHopSupervisor au talpid-core state machine

1. Lire `talpid-core/src/tunnel_state_machine/` (mod.rs +
   connecting/connected/disconnected/disconnecting/error states).
2. Brancher `warren_client::MultiHopSupervisor` (M4.E.D) au tunnel
   state machine : reconnect transparent quand exit drop.
   - Probable pattern : background task supervisor spawn dans
     `connecting_state.rs`, écoute reconnect events, transition
     state machine `connected → reconnecting → connected` sans
     remonter à l'UI.
   - Auto-reconnect doit incrémenter `reconnect_count` et exposer
     `last_reconnect_age` au daemon pour UI future M4.H.C.
3. TDD :
   - Test mid-session exit drop → supervisor triggers reconnect →
     tunnel reconnects → traffic resumes
   - Test multiple consecutive drops → backoff applied (Backoff::HANDSHAKE)
   - Test idle timeout → MultiHopSupervisor keep-alive prevents drop

### M4.H.B.5 - Tests régression cross-mode

1. `cargo test -p talpid-warren-tunnel` : doit passer 100% (single +
   multi).
2. `cargo test -p talpid-core` : régression state machine.
3. `cargo test -p mullvad-daemon` : régression query_from_settings,
   relay_selector, tunnel_params.
4. Cargo validation groupée fin de phase :
   ```bash
   cargo fmt --check &
   cargo clippy -p talpid-warren-tunnel -p talpid-core -p mullvad-daemon -p mullvad-relay-selector --all-targets -- -D warnings
   wait
   cargo test -p talpid-warren-tunnel -p talpid-core -p mullvad-daemon -p mullvad-relay-selector
   cargo check --workspace
   ```

### M4.H.B.6 - Cross-compile + deploy bench env

1. Cross-compile Linux x86_64 release `mullvad-daemon` + `mullvad-cli`.
2. Provision Hetzner :
   - 1× CCX23 nbg1 (client warren-app daemon-fork bench)
   - 1× CCX23 fra1 (relay : warren-relay binaire depuis warren-core
     HEAD si pas déjà déployé prod)
   - warren-exit-1 prod hel1 reused (HEAD+fix M4.H.A.quart)
3. scp parallèle binaires.
4. Mint relay descriptor + exit descriptor signed via wapi helpers
   admin (paths cf. memory `warren_prod_admin_key_location`
   warren-core).
5. Configure daemon-fork client avec multi_hop=true settings, exit
   country filter.

### M4.H.B.7 - Bench cross-mode

1. **Single-hop bench** (baseline rappel M4.H.A.quart) :
   - TCP 4-flow 5 min sustained cible ≥ 200 Mbps (M4.H.A.quart : 802
     Mbps réel)
   - 0 stall, 0 errors, RSS stable
2. **Multi-hop bench** :
   - TCP 4-flow 5 min sustained cible ≥ 70 Mbps (cf. doctrine multi-
     hop perf attendue 75-85% single-hop)
   - 0 stall, 0 errors, 0 HPKE decode failures
   - PMTU négocié ≥ 1280
   - reconnect_count = 0 (pas de drop spontané sur 5 min)
3. **Auto-reconnect bench** (M4.E.D validation côté warren-app) :
   - Lancer bench multi-hop 10 min
   - À T+3min : SSH warren-exit-1, `systemctl restart warren-exit`
   - Attendre reconnect transparent (médian 3s d'après M4.E.D, worst
     31s)
   - Bench reprend, vérifier reconnect_count incrémenté, traffic
     resumes
4. Tear-down nbg1 + fra1 transient (warren-exit-1 prod laissé UP).

### M4.H.B.8 - Finalize + commits + memory

1. Rapport `/tmp/m4-h-b-report.md` ≤ 200 lignes (§8).
2. Commits warren-app main :
   - `feat(talpid-warren-tunnel): add warren-multihop/client/relay/backoff path-deps`
   - `feat(mullvad-daemon): extend WarrenTunnelParameters with MultiHopConfig`
   - `feat(mullvad-daemon): adapter warren_relay_selector for multi-hop dispatch`
   - `feat(talpid-warren-tunnel): dispatch single/multi-hop in start_tunnel`
   - `feat(talpid-core): wire MultiHopSupervisor to state machine for transparent reconnect`
   - `bench(M4.H.B): cross-DC single + multi + auto-reconnect verdict`
   (commits atomiques par sous-tâche, push origin/main au fil de l'eau
   ou à la fin selon ta jugement)
3. Memory update warren-app : `warren_m4h_b_delivered.md` + index
   MEMORY.md.

---

## 5. Règles non-négociables

### Sécurité

- Pas de secrets verbatim (cf. `feedback_warren_no_secrets_in_commits`).
- Pas de signing key prod touchée. Admin signing key warren-api =
  `~/.local/admin-stack/admin/admin-signing.key` (cf. memory
  `warren_prod_admin_key_location` warren-core).
- HPKE epoch rotation < 8h figée par doctrine (modifiable runtime mais
  pas /v1 figé < 8h).

### Code

- TDD strict (cf. warren-core CLAUDE.md §1 et `feedback_tests_pertinents`)
- /v1 constantes IMMUABLES (ALPN_H3, HKDF salt/info, AAD layout,
  descriptor schema, DIRECTION_TAG_REVERSE = 0x02 etc.). Si nécessaire
  → escalade /v2.
- Pas d'unsafe (forbid)
- Pas d'em-dash, pas de step-tracking, pas de TODO laissés
- Anglais comments, conventional commits subject-only

### Git

- Push main direct warren-app. Pas de feature branch.
- Si fix tactique cross-repo warren-core nécessaire pour débloquer :
  push main warren-core aussi (cf. doctrine §0.5).

### Bench Hetzner

- Tear-down nbg1 + fra1 obligatoire.
- warren-exit-1 prod RESTE UP avec HEAD+fix M4.H.A.quart.
- `hcloud server list` final = warren-exit-1 + warren-backend-api seuls.

---

## 6. Pas de validation intermédiaire poka

§0.5 ci-dessus. 4 cas d'escalade ONLY.

---

## 7. Critères phase livrée

### GO ULTIMATE (cible)

- Path-deps warren-multihop/client/relay/backoff câblés
- `WarrenTunnelParameters` étendu avec `Option<MultiHopConfig>`
- Dispatcher single/multi-hop opérationnel dans `talpid-warren-tunnel`
- `MultiHopSupervisor` branché au tunnel state machine
- TDD : tests régression single + multi + auto-reconnect tous PASS
- Cargo validation full : fmt + clippy -D warnings + test + check
  workspace = PASS
- Bench cross-DC :
  - Single-hop ≥ 200 Mbps TCP 4-flow (rappel baseline 802 Mbps)
  - Multi-hop ≥ 70 Mbps TCP 4-flow
  - Auto-reconnect transparent : reconnect_count ≥ 1, traffic resumes,
    UI ne flicker pas
  - 0 stall, 0 errors, 0 HPKE decode failures
  - PMTU ≥ 1280 sur les 2 modes
- Commits poussés
- Memory update warren-app

### GO CONDITIONAL

- Stack câblée + tests PASS mais :
  - Throughput multi-hop entre 40-70 Mbps OU
  - Auto-reconnect fonctionne mais médian > 10s OU
  - 1 caveat secondaire (ex: HPKE rotation < 8h pas testé empiriquement)

### NO-GO HONNÊTE (très improbable §0.5)

- Breaking change /v1 nécessaire (escalade)
- 4h+ investigation profonde sur bug archi sans progrès tangible
- Sécurité (signing key prod touchée nécessaire)

---

## 8. Rapport final attendu

`/tmp/m4-h-b-report.md` ≤ 200 lignes :

1. **Verdict** GO ULTIMATE / CONDITIONAL / NO-GO + 1 phrase
2. **Path-deps ajoutés** + SHA warren-core
3. **WarrenTunnelParameters extension** : diff schema + tests TDD
4. **warren_relay_selector adapt** : logique dispatch + tests
5. **Dispatcher single/multi** : architecture choisie + tests
6. **MultiHopSupervisor wiring** : état machine impact + tests
7. **Cargo validation** : output fmt + clippy + test résumé
8. **Bench results** tableau 3 modes (single + multi + auto-reconnect)
9. **Caveats résiduels** (notamment caveats M4.H.A.X persistants
   triés : daemon-fork Remote LOCAL=0, wapi VAL1/2, GHCR PAT)
10. **Coût Hetzner** + tear-down attesté
11. **Commits** + memory updates

---

## 9. Next steps post-phase (orchestrateur)

- **GO ULTIMATE** → débloque M4.H.C (UI Electron toggles + status :
  toggle multi-hop, choix exit country, reconnect_count display,
  killswitch IPv6+DNS, toggle obfuscation M4.0). Brief drafté à la
  livraison.
- **GO CONDITIONAL** → pondérer caveats, possible M4.H.C en parallèle.
- **NO-GO** → analyse cause root before M4.H.C.

Caveats M4.H.A.X persistants :
- daemon-fork `account create` Remote LOCAL=0 factory bug → fixable
  pendant M4.H.B vu que touche auth chain reorganisée. À traiter en
  scope opportuniste.
- wapi VAL1/2 client-side regression → fix tactique 30 min, à pousser
  warren-core pendant M4.H.B si rencontré.
- GHCR PAT poka-IT write:packages → ops poka, hors scope agent.

---

## 10. Trace de mémorisation

Warren-app :
- Create `warren_m4h_b_delivered.md`
- Update MEMORY.md : `- [M4.H.B delivered](warren_m4h_b_delivered.md), <verdict> + stack M4.E.D câblée + bench single/multi/auto-reconnect`

Warren-core (si cross-repo fix push) :
- Update memory si fix poussé sur warren-core (ex. wapi VAL1/2)

Optionnel mais souhaitable :
- Memory `project_warren_app_state_post_m4hb.md` warren-app actant
  l'état du dispatcher single/multi + branchement MultiHopSupervisor.
  Remplacera partiellement le memory `project_warren_app_state_post_quinn`
  comme source de vérité orchestrateur.
