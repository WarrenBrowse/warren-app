# Phase M4.H.F - NAT-PMP client câblage + UI port-forwarding (différenciateur produit)

> Brief d'agent autonome cross-repo warren-core + warren-app.
> Doctrine §0.5 full autonomy NO timid rollback.
> La commande `/goal` compagne pointe vers ce fichier.

**Effort estimé** : wall-clock 3-5 jours.
**Coût Hetzner** : 0-0.10 EUR (test mapping E2E optionnel si SSH
résolu).
**Pré-condition** :
- warren-app main HEAD `51cd25ad03+` (post-M4.H.D, hosting GitHub
  `WarrenBrowse/warren-app`)
- warren-core HEAD `a7159d94+` (post-M4.H.E observer pattern)
- Stack M4.E.D câblée + UI complète + build pipeline prêt

**Objectif** : câbler le différenciateur produit "port-forwarding
restauré" (vs Mullvad/IVPN abandon 2023 cf. `warren_product_corrected`).

1. **Warren-core** : ajouter un helper `refresh_loop` dans
   `warren-natpmp-client` pour auto-renewal lifetime (POC limit
   actuel : caller doit re-request manuellement).
2. **Warren-app daemon** : path-deps `warren-natpmp-client` +
   `warren-natpmp-protocol`, NatPmpConfig dans WarrenTunnelParameters,
   spawn refresh_loop quand tunnel UP, cleanup quand DOWN.
3. **Warren-app UI** : créer from scratch `port-forwarding-settings/`
   view Electron (Mullvad upstream a supprimé sa version 2023). Toggle
   + display port + lifetime countdown live + gRPC observer.
4. **gRPC proto** : NatPmpSettings + NatPmpStatus messages + 3 rpc
   (Get/Set + Updates stream).
5. **Tests TDD** : refresh loop warren-core (RED→GREEN), daemon-side
   NatPmpManager state, UI Jest, i18n FR+EN.

Test E2E empirique conditionné SSH Hetzner résolu (ou skip avec caveat
documenté).

---

## 0. MANDAT STRICT

Anti-patterns M4.E §7 + TDD strict warren-core CLAUDE.md §1
(RED→GREEN→REFACTOR) sur `refresh_loop` warren-core + tests Jest
warren-app UI. /v1 constantes IMMUABLES (RFC 6886 wire format est
externe immuable de facto). Pas em-dash, anglais comments, no-log
Warren (pas de log de l'IP publique exit user-side).

---

## 0.5 MANDAT D'AUTONOMIE

Cf. memory `feedback_agent_full_autonomy_no_timid_rollback`. Plein
mandat. Diagnostic 30 min → fix tactique TDD → commit + push →
reprise. PAS de rollback, PAS d'escalade timide.

Escalade `AskUserQuestion` SEULEMENT si :
1. Secret leak
2. Coût Hetzner > 0.30 EUR
3. Breaking change /v1 wire format (RFC 6886 = externe, jamais)
4. Signing key prod touchée
5. **Spécifique M4.H.F** : warren-natpmp-server warren-exit a un bug
   qui rend le mapping infonctionnel server-side (= scope hors phase,
   nécessite session warren-core dédiée)

NO-GO seulement si après 4h investigation + fix tentés un problème
dépasse l'agent (probablement archi refresh loop incompatible
state machine talpid-core).

Décisions tactiques agent autorisées :
- Architecture refresh_loop warren-core (callback observer vs channel)
- Lifetime default value (3600s recommandé)
- UI placement (sous vpn-settings ou top-level dans drawer)
- Comportement quand exit change (re-request OR carry lifetime ?)
- Display format port (5-digit dans badge OR full "Public port: 12345"
  in connection-details)

---

## 1. Optimisations agent

- Read sources cross-repo en PARALLÈLE
- Tests TDD groupés en fin de chaque sous-tâche
- Push warren-core + warren-app au fil de l'eau (8+ commits attendus
  cumulés)

---

## 2. Setup initial

```bash
cd /Users/poka/dev/warrenBros/warren-app
git status                                  # clean main 51cd25ad03+
git remote -v                                # origin = GitHub WarrenBrowse
cd /Users/poka/dev/warrenBros/warren-core
git status                                  # clean HEAD a7159d94+
git log --oneline -3
```

---

## 3. Sources à lire (PARALLÈLE)

### Warren-core

- `crates/warren-natpmp-client/src/lib.rs` (API `request_map`,
  `request_map_with_retries`, types `NatPmpMapping`)
- `crates/warren-natpmp-protocol/src/lib.rs` (RFC 6886 wire)
- `crates/warren-natpmp-server/src/lib.rs` (pour comprendre le
  comportement serveur attendu)
- `crates/warren-exit/src/main.rs` (vérifier le port UDP/5351 binding
  + lifetime max accepté)

### Warren-app

- `talpid-warren-tunnel/Cargo.toml` (path-deps à étendre)
- `talpid-warren-tunnel/src/lib.rs` (start_tunnel + start_multi_hop
  hooks pour spawn NAT-PMP refresh loop)
- `mullvad-daemon/src/warren_tunnel_params.rs` (WarrenTunnelParameters
  à étendre avec NatPmpConfig)
- `mullvad-daemon/src/management_interface.rs` (gRPC handlers ext)
- `mullvad-management-interface/proto/management_interface.proto`
  (extension proto)
- `mullvad-daemon/src/lib.rs` (state machine integration)
- `talpid-core/src/tunnel_state_machine/connected_state.rs`
  (post-handshake hook pour démarrer NAT-PMP)
- `talpid-core/src/tunnel_state_machine/disconnecting_state.rs`
  (cleanup NAT-PMP)
- `desktop/packages/mullvad-vpn/src/renderer/components/views/` 
  (référence views existants pour pattern, multihop-settings/ est un
  bon modèle)
- `desktop/packages/mullvad-vpn/src/main/daemon-rpc.ts` (gRPC client)
- `desktop/packages/mullvad-vpn/locales/messages.pot` + `fr/`

### Memory cross-session

- `warren_product_corrected.md` warren-core (différenciateur PF
  vs Mullvad/IVPN)
- `project_warren_app_state_post_m4hd.md` warren-app (ou plus récent,
  state-of-truth orchestrateur)
- `feedback_agent_full_autonomy_no_timid_rollback.md`
- `feedback_warren_competitor_comparatives.md` warren-core (comparatif
  doctrine 5 concurrents)

---

## 4. Plan d'exécution

### M4.H.F.0 - warren-core : refresh_loop NAT-PMP client

1. Lire `crates/warren-natpmp-client/src/lib.rs` actuel pour
   identifier extension points.
2. **TDD RED** : test que `refresh_loop(gateway, port_proto,
   port_internal, lifetime_secs, observer)` :
   - Appelle `request_map` à T=0 → mapping valide
   - Re-appelle `request_map` à T = lifetime_secs / 2 (renewal early)
   - Échec après N retries → callback observer notifié
   - Cancel handle stops the loop
3. **GREEN** : implémenter `refresh_loop` :
   - Tokio `tokio::time::sleep(lifetime_secs / 2)` between re-requests
   - Observer pattern via callback `Fn(NatPmpEvent)` ou channel
     `mpsc::Sender<NatPmpEvent>`
   - Events : `Mapped { port, lifetime }`, `Renewed { port, lifetime }`,
     `Failed { error }`, `Cancelled`
   - Handle `RefreshLoopHandle` avec `.cancel()` + `.join()`
4. Tests régression : timeout serveur, parse error, ResultCode error
5. Commit warren-core `feat(warren-natpmp-client): refresh_loop helper for automatic mapping renewal`
6. Push origin/main warren-core
7. Bumper `.warren-core-version` côté warren-app : commit
   `chore(warren-core-pin): bump for refresh_loop NAT-PMP helper`

### M4.H.F.1 - warren-app : path-deps + types

1. Étendre `talpid-warren-tunnel/Cargo.toml` :
   ```toml
   warren-natpmp-client = { path = "../../warren-core/crates/warren-natpmp-client" }
   warren-natpmp-protocol = { path = "../../warren-core/crates/warren-natpmp-protocol" }
   ```
2. `cargo check -p talpid-warren-tunnel` PASS.
3. Définir struct `NatPmpConfig` dans `mullvad-types` ou
   `talpid-warren-tunnel/src/lib.rs` :
   ```rust
   pub struct NatPmpConfig {
       pub enabled: bool,
       pub lifetime_secs: u32,        // default 3600
       pub protocol: NatPmpProto,     // TCP, UDP, both
   }
   ```
4. Étendre `WarrenTunnelParameters` daemon-side :
   ```rust
   pub struct WarrenTunnelParameters {
       // existing
       pub nat_pmp: Option<NatPmpConfig>,
   }
   ```
5. TDD : tests parse settings + default OFF.
6. Commit `feat(warren-tunnel): add NatPmpConfig in WarrenTunnelParameters`.

### M4.H.F.2 - daemon-side NatPmpManager state

1. Créer `mullvad-daemon/src/warren_nat_pmp.rs` :
   - `NatPmpManager` struct qui détient le `RefreshLoopHandle` actuel
   - `start(gateway: Ipv4Addr, config: NatPmpConfig, observer: ...)`:
     spawn refresh_loop warren-natpmp-client
   - `stop()` : cancel + drop handle
   - `current_mapping() -> Option<NatPmpMapping>` : query
2. Brancher au tunnel state machine `talpid-core` :
   - `connected_state.rs` post-handshake : si `nat_pmp.enabled` →
     `NatPmpManager.start(10.66.0.1, config, observer)`
   - `disconnecting_state.rs` : `NatPmpManager.stop()`
3. Observer pattern : NatPmpManager push events vers
   `WarrenStatusCache` (réutiliser le pattern M4.H.E.1 reconnect cache
   wiring)
4. TDD :
   - NatPmpManager start/stop idempotent
   - Observer receive Mapped event sur successful request_map mock
   - State transition triggers start/stop
5. Commit `feat(mullvad-daemon): NatPmpManager state lifecycle with refresh loop`.

### M4.H.F.3 - gRPC proto NatPmpSettings + Status

1. Étendre `management_interface.proto` :
   ```proto
   message NatPmpSettings {
       bool enabled = 1;
       uint32 lifetime_secs = 2;   // default 3600
       enum Proto { TCP = 0; UDP = 1; BOTH = 2; }
       Proto protocol = 3;
   }

   message NatPmpStatus {
       enum State { DISABLED = 0; REQUESTING = 1; MAPPED = 2;
                    EXPIRED = 3; FAILED = 4; }
       State state = 1;
       optional uint32 public_port = 2;
       optional uint32 lifetime_remaining_secs = 3;
       optional string error_message = 4;
   }

   // 3 nouveaux rpc :
   rpc GetNatPmpSettings(google.protobuf.Empty) returns (NatPmpSettings);
   rpc SetNatPmpSettings(NatPmpSettings) returns (google.protobuf.Empty);
   rpc NatPmpStatusUpdates(google.protobuf.Empty) returns (stream NatPmpStatus);
   ```
2. Regen TS bindings via podman.
3. Daemon-side handlers gRPC dans `management_interface.rs` (pattern
   M4.H.C.1 WarrenStatusUpdates).
4. TDD daemon handlers.
5. Commit `feat(management-interface): NAT-PMP settings + status proto messages`.
6. Commit `feat(mullvad-daemon): gRPC handlers for NAT-PMP settings + status stream`.

### M4.H.F.4 - UI Electron port-forwarding-settings view

Mullvad upstream a supprimé son UI PF en 2023 (cf. différenciateur
Warren). Créer from scratch en s'inspirant du pattern
`multihop-settings/`.

1. Créer `desktop/packages/mullvad-vpn/src/renderer/components/views/port-forwarding-settings/` :
   - `PortForwardingSettings.tsx` : view principale
   - `PortForwardingSettings.test.tsx` : tests Jest
   - `index.ts` exports
2. Composants :
   - Toggle "Port forwarding" (OFF par défaut)
   - Si ON : Lifetime selector (1h, 6h, 24h dropdown via settings-listbox)
   - Si ON : Display "Public port: 12345" (ou "Requesting..." pendant
     setup, "Failed: <err>" sur erreur)
   - Live countdown lifetime remaining (mm:ss format)
3. Connect gRPC via `daemon-rpc.ts` :
   - `GetNatPmpSettings` / `SetNatPmpSettings`
   - Stream `NatPmpStatusUpdates` pour live updates
4. Naviguer depuis settings principal (ajouter entry dans `settings/`
   ou directement sous vpn-settings selon choix tactique).
5. Tests Jest : toggle state, lifetime selection, live status display,
   countdown.
6. Commit `feat(desktop): port-forwarding-settings view (Warren differentiator vs Mullvad/IVPN abandon 2023)`.

### M4.H.F.5 - I18n FR + EN

1. Strings ajoutées (estimation 10-15) :
   - "Port forwarding"
   - "Lifetime"
   - "Public port: {port}"
   - "Requesting port mapping..."
   - "Mapping failed: {error}"
   - "Renewing in {countdown}"
   - "Port forwarding restored - differentiator unique vs major
     competitors who abandoned this feature in 2023"
   - etc.
2. `messages.pot` extract.
3. `locales/fr/messages.po` translations.
4. Commit `i18n(desktop): FR + EN strings for port-forwarding-settings`.

### M4.H.F.6 - Validation full

1. Côté warren-core :
   ```bash
   cd /Users/poka/dev/warrenBros/warren-core
   ./scripts/dev/cargo-test-nofw.sh fmt --check &
   ./scripts/dev/cargo-test-nofw.sh clippy -p warren-natpmp-client --all-targets -- -D warnings
   wait
   ./scripts/dev/cargo-test-nofw.sh test -p warren-natpmp-client -p warren-natpmp-protocol
   ```
2. Côté warren-app :
   ```bash
   cargo fmt --check &
   cargo clippy --workspace --all-targets -- -D warnings
   wait
   cargo test --workspace --no-fail-fast
   cd desktop && npm run lint && npm run test && npm run build
   ```
3. Smoke `bash scripts/dev/smoke-build.sh` PASS (vérifier que build.sh
   Warren-app intègre toujours OK avec les nouveaux modules).
4. Si SSH Hetzner résolu : test E2E empirique single-hop :
   - Provision CCX23 nbg1 client
   - Connect daemon → exit, enable NAT-PMP toggle, vérifier port
     assigné côté UI + sur l'exit `warren-exit --multihop` log
   - Si non résolu : skip avec caveat documenté

### M4.H.F.7 - Finalize + commits + memory

1. Rapport `/tmp/m4-h-f-report.md` ≤ 200 lignes.
2. Commits 8+ poussés origin/main (warren-app + warren-core).
3. Memory `warren_m4h_f_delivered.md` warren-app + index MEMORY.md.
4. Memory `warren_natpmp_client_wired.md` warren-core si pertinent
   (pour acter que `refresh_loop` est utilisé par warren-app daemon).
5. Update source-of-truth orchestrateur.

---

## 5. Règles non-négociables

### Sécurité

- Pas de log du port assigné par NAT-PMP côté logs daemon (= port =
  potentiellement identifiable, no-log Warren strict)
- Affichage UI OK (user a le droit de voir SON port assigné)
- Pas de secrets verbatim

### Code

- TDD strict (RED→GREEN→REFACTOR)
- /v1 RFC 6886 = wire format externe, jamais modifier le module
  warren-natpmp-protocol (juste consommer)
- Conventional commits subject-only, anglais comments, no em-dash

### Git

- Push main warren-app (GitHub WarrenBrowse) + warren-core direct
- Pas de feature branch
- Pin `.warren-core-version` bumped au début de phase

---

## 6. Pas de validation intermédiaire poka

§0.5. 5 cas escalade ONLY.

---

## 7. Critères phase livrée

### GO ULTIMATE (cible)

- **M4.H.F.0** : `refresh_loop` warren-core implémenté + tests
  RED→GREEN + push warren-core + pin bumped warren-app
- **M4.H.F.1** : path-deps + NatPmpConfig + WarrenTunnelParameters
  étendu + tests
- **M4.H.F.2** : NatPmpManager daemon-side state lifecycle + branché
  tunnel state machine + tests TDD
- **M4.H.F.3** : gRPC proto + handlers + TS bindings regen + tests
- **M4.H.F.4** : UI view créée from scratch + tests Jest + connect
  gRPC stream
- **M4.H.F.5** : i18n FR + EN (10-15 strings)
- Cargo validation warren-app + warren-core PASS (fmt + clippy
  -D warnings + test)
- npm validation desktop/ PASS (lint + test + build)
- 8+ commits atomiques poussés origin/main des 2 repos
- Test E2E empirique PASS si SSH Hetzner résolu (sinon caveat
  documenté)
- Memory updates + index

### GO CONDITIONAL

- Stack câblée + tests PASS mais :
  - Test E2E empirique skipped (SSH Hetzner caveat hérité)
  - OU 1 caveat secondaire (auto-renewal observable mais pas testé
    en mocked-time test edge case)

### NO-GO HONNÊTE (improbable §0.5)

- warren-natpmp-server warren-exit a un bug fondamental qui rend le
  mapping serveur-side cassé (= hors scope, session warren-core)
- 4h+ investigation sans progrès sur state machine integration

---

## 8. Rapport final attendu

`/tmp/m4-h-f-report.md` ≤ 200 lignes :

1. **Verdict** GO ULTIMATE / CONDITIONAL / NO-GO
2. **refresh_loop warren-core** : SHA commit, tests RED snippet,
   architecture choisie (callback vs channel)
3. **NatPmpManager daemon** : architecture lifecycle, tests TDD
4. **gRPC proto extension** : diff schema + TS regen
5. **UI view** : structure files, tests Jest count
6. **I18n** : nb strings FR + EN
7. **Validation cargo + npm** : output summary
8. **Test E2E** : empirique PASS, ou caveat SSH Hetzner différé
9. **Différenciateur produit** : note marketing-ready (cf.
   `feedback_warren_competitor_comparatives` : "Warren = seul VPN
   consumer FR avec port-forwarding restauré en 2026")
10. **Commits** + memory updates

---

## 9. Next steps post-phase (orchestrateur)

- **GO ULTIMATE** → débloque M4.H.G (--bypass-cidr + backoff tune,
  ~1j) puis M4.H.H (doc warrenbrowse.com, scope web séparé)
- **CONDITIONAL** → pondérer
- **NO-GO** → analyse cause root before next

Caveats persistants post-M4.H.F :
- SSH Hetzner (toujours, ops poka)
- GH Actions billing + signing assets + WARREN_CORE_RO_TOKEN
  (ops poka, M4.H.D heritage)

Ship readiness check post-M4.H.F :
- Beta release Warren possible quand : caveats ops poka résolus +
  premier vrai tag release.yml triggered + bench installer empirique
  PASS

---

## 10. Trace de mémorisation

Warren-app :
- Create `warren_m4h_f_delivered.md`
- Update source-of-truth orchestrateur
- Index MEMORY.md : `- [M4.H.F delivered](warren_m4h_f_delivered.md) — <verdict> NAT-PMP port-forwarding différenciateur produit (vs Mullvad/IVPN abandon 2023)`

Warren-core :
- Si M4.H.F.0 push : update memory warren-core si pertinent
  (`warren_natpmp_client_refresh_loop.md` documentant le helper +
  premier consumer warren-app)
