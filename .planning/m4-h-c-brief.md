# Phase M4.H.C - UI Electron Warren : multi-hop toggle + reconnect status + killswitch + obfuscation indicator

> Brief d'agent autonome warren-app (Electron React TypeScript + gRPC
> + daemon Rust). Doctrine NOUVELLE §0.5 full autonomy NO timid
> rollback. La commande `/goal` compagne pointe vers ce fichier.

**Effort estimé** : wall-clock 4-6 jours (révisé baisse car composants
upstream Mullvad existent déjà : `multihop-settings/`, `daita-settings/`,
`anti-censorship/`, `vpn-settings/`).
**Coût Hetzner** : 0 EUR (UI work, pas de bench Hetzner dans cette phase).
**Pré-condition** :
- warren-app main HEAD `b9ae8e050e` (post-M4.H.B) ou descendant
- Stack M4.E.D câblée daemon-side (warren_multi_hop.rs +
  warren_multi_hop_mode.rs + warren_tunnel_params.rs OK)
- Volta + node + podman installés (cf. BuildInstructions.md)

**Objectif** : exposer dans l'UI Electron Warren les features stack
M4.E.D câblées par M4.H.B :
1. Toggle multi-hop ON/OFF (OFF default, doctrine
   `warren_multihop_doctrine_v1`)
2. Sélection exit country + entry country (si multi-hop ON)
3. Status reconnect_count + last_reconnect_age (M4.E.D auto-reconnect)
4. Toggle killswitch IPv6 + DNS leak
5. Indicator obfuscation M4.0 (always-on /v1, info-only display)
6. I18n FR + EN sur toutes les nouvelles strings

Adapter les composants upstream Mullvad existants (`multihop-settings/`,
`vpn-settings/`, `anti-censorship/`) pour la sémantique Warren plutôt
que créer from scratch.

---

## 0. MANDAT STRICT

Anti-patterns historiques M4.E §7 + TDD strict (côté Rust daemon
modifications et tests Jest/Vitest côté UI) + /v1 constantes IMMUABLES
(escalade /v2). React composants doivent suivre les patterns existant
upstream Mullvad (settings-listbox, settings-list-item, settings-
accordion).

---

## 0.5 MANDAT D'AUTONOMIE (cf. memory `feedback_agent_full_autonomy_no_timid_rollback`)

**Tu as plein mandat.** Si imprévu :
- Diagnostic 30 min max
- Fix tactique TDD strict (cross-repo OK)
- Commit + push origin/main
- Reprise procédure

PAS de rollback opportuniste. PAS d'escalade "pour valider l'approche
UI" / "que faire pour i18n FR" / "quel composant choisir". TU DÉCIDES.

Escalade `AskUserQuestion` SEULEMENT si :
1. Secret leak
2. Coût > 0.30 EUR (n/a cette phase pas de Hetzner)
3. Breaking change /v1 wire format
4. Signing key prod touchée

Décisions tactiques que tu peux prendre seul :
- Naming des nouvelles strings i18n
- Hiérarchie des settings (où placer le toggle multi-hop : sous
  vpn-settings ou top-level ?)
- Style visuel des indicators (badge, icône, banner)
- Choix dropdown vs listbox pour country selection

---

## 1. Optimisations agent

- Lectures sources en parallèle (UI components existants + daemon
  Rust + proto)
- Edit batch puis cargo + npm test groupé en fin de sous-tâche
- npm run build une seule fois en validation phase
- pas de "npm install" entre chaque edit
- Hot-reload Electron dev mode si disponible

---

## 2. Setup initial

```bash
cd /Users/poka/dev/warrenBros/warren-app
git status                                  # clean main HEAD b9ae8e050e+
git log --oneline -3
node --version                              # via Volta
cd desktop && volta list 2>&1 | head
cd /Users/poka/dev/warrenBros/warren-app
ls scripts/dev/                             # dev wrappers
```

Si Volta/node manquant : escalade poka (setup prérequis). Volta
managé via `desktop/package.json`.

---

## 3. Sources à lire (PARALLÈLE)

### Composants UI Electron upstream à adapter

- `desktop/packages/mullvad-vpn/src/renderer/components/views/multihop-settings/`
  (Mullvad multihop natif WG, point de départ)
- `desktop/packages/mullvad-vpn/src/renderer/components/views/vpn-settings/`
  (toggle killswitch, obfuscation existante)
- `desktop/packages/mullvad-vpn/src/renderer/components/views/anti-censorship/`
  (obfuscation Mullvad, pattern toggle)
- `desktop/packages/mullvad-vpn/src/renderer/components/views/daita-settings/`
  (DAITA toggle pattern, modèle pour Warren multi-hop toggle)
- `desktop/packages/mullvad-vpn/src/renderer/components/views/select-location/`
  (location chooser composant)
- `desktop/packages/mullvad-vpn/src/renderer/components/views/keys/`
  (Warren BIP39 keys view, déjà adapté R1)
- `desktop/packages/mullvad-vpn/src/renderer/components/settings-listbox/`
- `desktop/packages/mullvad-vpn/src/renderer/components/settings-list-item/`
- `desktop/packages/mullvad-vpn/src/renderer/components/settings-accordion/`
- `desktop/packages/mullvad-vpn/src/renderer/lib/account.ts` (NB: ce
  fichier ancien, vérifier qu'il n'est pas déjà supprimé R1)

### IPC + daemon-side

- `mullvad-management-interface/proto/management_interface.proto`
  (à étendre pour exposer Warren multi-hop + reconnect_count)
- `mullvad-management-interface/proto/relay_selector.proto`
- `mullvad-daemon/src/management_interface.rs` (handlers gRPC daemon-
  side, à étendre)
- `mullvad-daemon/src/warren_multi_hop.rs` (PKI loader, déjà M4.H.B)
- `mullvad-daemon/src/warren_multi_hop_mode.rs` (toggle mode, déjà M4.H.B)
- `mullvad-daemon/src/warren_tunnel_params.rs` (params extended M4.H.B)
- `mullvad-daemon/src/warren_relay_selector.rs` (sélection relay+exit)
- `mullvad-daemon/src/warren_relay_list_view.rs` (vue GUI relays)
- `desktop/packages/mullvad-vpn/src/main/daemon-rpc.ts` (IPC client
  Electron→daemon)

### I18n

- `desktop/packages/mullvad-vpn/locales/messages.pot` (master)
- `desktop/packages/mullvad-vpn/locales/fr/` (FR strings)

### Memory cross-session

- `project_warren_app_state_post_m4hb.md` warren-app (source-of-truth
  orchestrateur post-M4.H.B)
- `warren_multihop_doctrine_v1.md` warren-core (toggle OFF default,
  pattern Apple Private Relay, two-relayed QUIC + HPKE)
- `warren_obfuscation_doctrine_v1.md` warren-core (M4.0 always-on /v1
  PAS de toggle, info-only display)
- `warren_daita_doctrine_v1.md` warren-core (DAITA OFF default, M4.G
  futur, pas dans scope M4.H.C)
- `feedback_agent_full_autonomy_no_timid_rollback.md` warren-app
  CRITIQUE
- `feedback_warren_phase_prompts_no_branch.md` warren-core
- Memory `warren_m4h_b_delivered.md` warren-app (état post-M4.H.B,
  Map des modifs daemon-side)

---

## 4. Plan d'exécution

### M4.H.C.0 - gRPC proto extension

1. Lire `management_interface.proto` actuel pour identifier le pattern
   existant (services, messages).
2. Étendre avec Warren multi-hop fields :
   ```proto
   message WarrenMultiHopSettings {
       bool enabled = 1;
       string entry_country = 2;  // ISO 3166 code
       string exit_country = 3;
       google.protobuf.Duration hpke_epoch_rotation = 4;  // default 4h
   }

   message WarrenStatus {
       uint32 reconnect_count = 1;
       google.protobuf.Duration last_reconnect_age = 2;
       bool obfuscation_active = 3;  // M4.0 always-on /v1
   }

   service ManagementService {
       // existing rpc...
       rpc GetWarrenMultiHopSettings(google.protobuf.Empty)
           returns (WarrenMultiHopSettings);
       rpc SetWarrenMultiHopSettings(WarrenMultiHopSettings)
           returns (google.protobuf.Empty);
       rpc GetWarrenStatus(google.protobuf.Empty)
           returns (WarrenStatus);
       rpc WarrenStatusUpdates(google.protobuf.Empty)
           returns (stream WarrenStatus);  // push updates UI
   }
   ```
3. Regen TS bindings : podman container Mullvad existant (cf.
   BuildInstructions.md gRPC codegen). Si rate de regen : escalade.
4. Commit `feat(management-interface): add Warren multi-hop + status proto messages`.

### M4.H.C.1 - Daemon-side handlers gRPC

1. Étendre `mullvad-daemon/src/management_interface.rs` :
   - Implement `GetWarrenMultiHopSettings` : lire depuis settings
     persistés (warren_query_from_settings + warren_multi_hop_mode)
   - Implement `SetWarrenMultiHopSettings` : update settings + push
     event au tunnel state machine pour reconnect avec nouveau mode
   - Implement `GetWarrenStatus` : query reconnect_count +
     last_reconnect_age depuis MultiHopSupervisor + obfuscation_active
     (M4.0 always-on = true)
   - Implement `WarrenStatusUpdates` stream : observer pattern pour
     push updates UI quand reconnect_count change ou state machine
     transition
2. TDD :
   - Test get default settings = multi-hop OFF + entry/exit empty
   - Test set settings persists + triggers reconnect event
   - Test status query returns correct reconnect_count
   - Test stream emits on reconnect event
3. Commit `feat(mullvad-daemon): expose Warren multi-hop settings + status via gRPC`.

### M4.H.C.2 - UI multihop-settings adaptation

1. Lire le view `multihop-settings/` existant Mullvad pour identifier
   sa structure (probable : toggle ON/OFF + entry location picker +
   exit location picker).
2. Forker ou adapter le view pour Warren :
   - Toggle "Warren multi-hop" (OFF default, message d'info doctrine
     Apple Private Relay)
   - Si ON : afficher entry country selector + exit country selector
     (different)
   - Si OFF : afficher info "Single-hop active, full bandwidth"
   - Connect au daemon via `daemon-rpc.ts` (GetWarrenMultiHopSettings
     + SetWarrenMultiHopSettings)
3. Tests Jest/Vitest pour le composant (toggle state, country
   selection, gRPC call mocked).
4. Commit `feat(desktop): adapt multihop-settings view for Warren two-relayed QUIC`.

### M4.H.C.3 - Status reconnect display dans connection details

1. Identifier le view existant connection-details ou similar (search
   `desktop/packages/mullvad-vpn/src/renderer/components/`).
2. Ajouter section "Warren auto-reconnect" :
   - Label reconnect_count (e.g. "Reconnects: 3 since session start")
   - Label last_reconnect_age (e.g. "Last reconnect: 12s ago" si > 0)
3. Connect au stream `WarrenStatusUpdates` pour live updates.
4. Tests Jest.
5. Commit `feat(desktop): display Warren reconnect_count + last_reconnect_age`.

### M4.H.C.4 - Killswitch IPv6 + DNS leak toggle

1. Lire `vpn-settings/` existant Mullvad : probable toggle killswitch
   déjà présent. Étendre avec :
   - Toggle "Block IPv6" (recommended ON, doctrine M4.E.C IPv6
     killswitch validé)
   - Toggle "Block DNS leaks" (recommended ON, F11+Round25 validés)
2. Daemon-side : si pas déjà exposé, ajouter handlers gRPC pour ces
   toggles.
3. TDD côté daemon + tests Jest UI.
4. Commit `feat(desktop): expose Warren killswitch IPv6 + DNS leak toggles in vpn-settings`.

### M4.H.C.5 - Obfuscation M4.0 indicator (info-only)

1. Lire `anti-censorship/` view existant Mullvad : toggle Shadowsocks
   / QUIC-over-TCP / LWO. **Warren ne propose PAS de toggle ici** car
   M4.0 obfuscation HTTP/3 mimicry est always-on /v1 (doctrine
   `warren_obfuscation_doctrine_v1`).
2. Adapter le view pour afficher un info-banner Warren :
   - "HTTP/3 mimicry obfuscation: always-on (ALPN h3 + SNI
     warrenbrowse.com + Initial split + port 443)"
   - Lien doc "Why is this always on?"
3. Désactiver/cacher les toggles Mullvad obsolètes (Shadowsocks, etc.)
   qui ne s'appliquent pas à Warren.
4. Tests Jest.
5. Commit `feat(desktop): replace anti-censorship view with Warren M4.0 always-on indicator`.

### M4.H.C.6 - I18n FR + EN

1. Extract toutes nouvelles strings ajoutées (Warren multi-hop, status,
   killswitch, obfuscation) → `messages.pot`.
2. Add FR translations dans `locales/fr/messages.po`.
3. Vérifier EN strings cohérentes (no Mullvad branding résidu).
4. Commit `i18n(desktop): FR + EN strings for Warren multi-hop + status + killswitch`.

### M4.H.C.7 - Validation full

1. Côté daemon :
   ```bash
   cargo fmt --check &
   cargo clippy --workspace --all-targets -- -D warnings
   wait
   cargo test --workspace --no-fail-fast
   cargo check --workspace
   ```
2. Côté UI :
   ```bash
   cd desktop
   npm run lint
   npm run test
   npm run build
   ```
3. Si dev env permet : `npm run start` Electron dev mode + smoke
   manuel des 5 features ajoutées (toggle multi-hop, country picker,
   reconnect display, killswitch, obfuscation indicator).

### M4.H.C.8 - Finalize + commits + memory

1. Rapport `/tmp/m4-h-c-report.md` ≤ 200 lignes.
2. Push origin/main warren-app (commits atomiques accumulés).
3. Memory `warren_m4h_c_delivered.md` + index MEMORY.md.

---

## 5. Règles non-négociables

### Sécurité

- Pas de secrets verbatim (cf. `feedback_warren_no_secrets_in_commits`).
- Pas de logging pubkey complète, IP utilisateur, nonce en clair côté
  UI (rule no-log Warren).

### Code

- TypeScript strict, pas de `any` ajouté
- React hooks pattern existant (useEffect, useState, useCallback)
- Rust TDD strict côté handlers gRPC
- Pas d'em-dash, anglais comments code, conventional commits
- /v1 constantes IMMUABLES (proto schema = wire, mais ajout non-
  breaking OK : nouveaux champs avec tags > existants)

### Git

- Push main direct warren-app, pas de feature branch.
- Conventional commits subject-only.

---

## 6. Pas de validation intermédiaire poka

§0.5. 4 cas d'escalade ONLY.

---

## 7. Critères phase livrée

### GO ULTIMATE (cible)

- gRPC proto étendu + TS bindings regen + daemon handlers
  implémentés + tests TDD PASS
- View `multihop-settings/` adapté pour Warren (toggle + entry + exit
  picker)
- Reconnect status display fonctionnel (count + age)
- Killswitch IPv6 + DNS toggles exposés
- Obfuscation M4.0 indicator info-only banner
- I18n FR + EN complétés sur nouvelles strings
- cargo + npm validation pipeline PASS
- Si dev env permet : smoke manuel UI 5 features PASS
- Commits poussés + memory update

### GO CONDITIONAL

- 4/5 features livrées, 1 caveat documenté (ex: stream
  WarrenStatusUpdates non testé empiriquement faute de bench env)

### NO-GO HONNÊTE (improbable §0.5)

- Breaking change /v1 proto nécessaire
- 4h+ investigation sans progrès sur gRPC codegen ou Electron build

---

## 8. Rapport final attendu

`/tmp/m4-h-c-report.md` ≤ 200 lignes :

1. **Verdict** GO ULTIMATE / CONDITIONAL / NO-GO
2. **gRPC proto extension** : diff schema + TS bindings regen
3. **Daemon handlers** : list + tests TDD count
4. **UI views adaptés** : list + tests Jest count
5. **I18n stats** : nb strings FR + EN ajoutées
6. **Validation pipeline** : output cargo + npm summary
7. **Smoke manuel (si applicable)** : 5 features statut
8. **Caveats** résiduels (notamment caveats M4.H.A.X + M4.H.B caveat
   SSH Hetzner persistants)
9. **Commits** + memory updates

---

## 9. Next steps post-phase (orchestrateur)

- **GO ULTIMATE** → débloque M4.H.D (build pipeline DMG macOS +
  AppImage Linux + MSI Windows + signing keys Warren + CI release)
  + M4.H.E (`--bypass-cidr` SSH fix + backoff tune)
- Caveats M4.H.A.X persistants :
  - daemon-fork Remote LOCAL=0 factory bug → opportuniste M4.H.C si
    touche auth chain UI
  - wapi VAL1/2 → fix indep warren-core
  - GHCR PAT poka-IT write:packages → ops poka
- Caveat M4.H.B SSH Hetzner → ops poka pour bench E2E final M4.H.D

---

## 10. Trace de mémorisation

Warren-app :
- Create `warren_m4h_c_delivered.md`
- Update `project_warren_app_state_post_m4hb.md` → renommer en
  `project_warren_app_state_post_m4hc.md` OU ajouter section
  "post-M4.H.C UI exposed"
- Index MEMORY.md : `- [M4.H.C delivered](warren_m4h_c_delivered.md) — <verdict> UI Electron Warren multi-hop + reconnect + killswitch + obfuscation indicator`
