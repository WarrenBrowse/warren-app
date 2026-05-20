# Session B - rapport intermédiaire

**Date** : 2026-05-20
**Statut global** : **PARTIEL** - DAITA v2 groundwork posé, B.2 / B.3 non démarrés
**Push** : warren-core `ab34ab5` (origin/main)
**Cwd réel d'exécution** : `/Users/poka/dev/warrenBros/warren-core`

---

## Synthèse honnête de scope

Le brief estimait 4-5 semaines wall-clock pour B.1 + B.2 + B.3. Cette
session a couvert la phase de research + decision archi + intégration
framework DAITA + tests. Les phases B.2 (failover) et B.3 (onboarding)
**ne sont pas démarrées**. La phase B.1 n'est pas terminée non plus :
le wrapper framework existe, mais l'intégration pump (timer driver, dummy
packet emission, multi-hop HPKE pre-encryption) reste à faire.

C'est moins que le brief mais c'est de la production-quality
prête-à-construire-dessus : 10 TDD verts, clippy strict, fmt, doc, push.

---

## B.1 - DAITA v2 framework

### B.1.0 - Research + decision archi (DONE)

Verrous appris du recensement web (sources : `mullvad.net/en/blog/daita-version-2-now-available-on-all-platforms`, `pulls.name/blog/2025-03-27-daita-v1-and-v2-defenses/`, `github.com/maybenot-io/maybenot`) :

1. **DAITA v2 n'est pas un set fixe de machines**. C'est une architecture
   **server-negotiated** : le relay/exit sélectionne par session une machine
   d'une database de défenses pré-générées (Mullvad parle de "thousands of
   configurations"), envoie la spec au client au handshake, les deux côtés
   instancient un framework `maybenot 2.x` avec cette machine.
2. **Mullvad n'a pas encore publié** le protocole de négociation v2 exact
   ni le pipeline `maybenot-gen` de génération des "thousands of defenses".
   Un paper PETS 2026 est en flight (Tobias Pulls : *"We are wrapping up
   an academic paper... we will release an open-source library and a cli-tool
   for creating them"*).
3. **`maybenot` 2.2.2** (2025-09-12) est le framework public sur crates.io,
   MIT/Apache-2.0 (compat GPL/AGPL Warren). C'est la même fondation que
   DAITA v2 chez Mullvad (qui l'embarque via `maybenot-ffi` dans
   wireguard-go). Warren-app dépend déjà de `maybenot-ffi 2.2.2` pour
   l'intégration wireguard-go côté Mullvad mais ce n'est pas utilisable
   pour le tunnel Quinn Warren.
4. **`maybenot-machines` 1.0.1** expose 8+ défenses hand-crafted issues
   de la littérature WF académique : NoOp, SimpleNetFlow, RegulaTor,
   Tamaraw, FRONT, Interspace, BreakPad, Scrambler. C'est notre pool
   initial.
5. **ALPN bump vs PROTOCOL_VERSION bump** : poka a sélectionné "bump /v2
   ALPN majeur" mais Warren utilise `ALPN_H3` (HTTP/3 mimicry M4.0
   obfuscation, cf. `warren_obfuscation_doctrine_v1`). Un ALPN custom
   `warren/exit/2` casserait l'obfuscation. La bonne option mécaniquement
   = **bump PROTOCOL_VERSION 2→3 in-band** (warren-protocol already
   structured for lockstep upgrade via `deny_unknown_fields`). Intent
   breaking respecté, ALPN H3 préservé.

### B.1.1 - Dependencies maybenot (DONE)

```toml
# workspace Cargo.toml
maybenot = "2.2.2"
maybenot-machines = "1.0.1"

# crates/warren-tunnel/Cargo.toml
maybenot = { workspace = true }
maybenot-machines = { workspace = true }
rand_v9 = { package = "rand", version = "0.9" }  # alias maybenot wants rand 0.9
```

`rand_v9` cohabite avec workspace `rand 0.10` ; aucune contagion hors
de `daita.rs`. À retirer quand maybenot bump rand 0.10.

### B.1.2 - Pool machines initial (PARTIEL)

`maybenot-machines` 1.0.1 fournit `get_machine(&[StaticMachine], &mut rng)`.
**Set "initial 5"** à pré-générer côté exit (NOT WIRED YET) :

- `StaticMachine::SimpleNetFlow` (Tor NetFlow padding, padding [1.5s,9.5s])
- `StaticMachine::Tamaraw { p: 5.0, stop_window: 1_000_000.0 }` (constant-rate)
- `StaticMachine::Front { padding_budget_max: 1500, window_min: 1.0, window_max: 10.0, num_states: 200 }`
- `StaticMachine::Interspace{Client,Server}` (random bursting)
- `StaticMachine::Scrambler{Client,Server { interval: 50.0, min_count: 4.0, min_trail: 4.0, max_trail: 16.0 }}`

À sélectionner aléatoirement par session côté exit, embarqué dans le
`SetupAck` v3. **PAS encore wired** dans `warren-exit`.

### B.1.3 - Pump integration (PAS FAIT)

Le `DaitaFramework` est instancié-able des deux côtés mais l'intégration
réelle dans `crates/warren-tunnel/src/pump.rs` reste à faire :

- Spawn d'une tokio task "DaitaDriver" qui `select!` entre :
  - événements `mpsc<DaitaEvent>` poussés par pump_*
  - timer wheel pour les `DaitaAction::SendPadding { timeout, ... }`
  - timer wheel pour les `DaitaAction::BlockOutgoing { timeout, ... }`
- Hook dans `pump_tun_to_quic` + `pump_quic_to_tun` + `pump_bidirectional`
  + `pump_multi_bidirectional` + variantes rate-limited (4 paths).
- Dummy datagram émission : datagram Quinn avec **first byte 0xFF** (=
  premier nibble 0xF, qui n'est ni 0x4 IPv4 ni 0x6 IPv6, donc
  non-breaking wire format `/v1` data plane - le kernel TUN dropperait
  déjà silencieusement un tel paquet, on ajoute juste un drop explicite
  early-return côté pump avant le tun.send).
- Receiver pump : si `dg[0] >> 4` not in `{4, 6}` → fire `PaddingRecv` +
  drop, sinon fire `NormalRecv` + tun.send.

Capté comme task #13 (`B.1.3.2 DaitaDriver async task (pump integration)`).

### B.1.4 - ALPN /v2 OU PROTOCOL_VERSION 3 (PAS FAIT)

Décision archi : **PROTOCOL_VERSION 2 → 3** in-band (préserve obfuscation
M4.0). Setup gagne `daita_support: bool`, SetupAck gagne
`daita_spec: Option<WarrenDaitaSpec>` portant `DaitaConfig`. Lockstep
upgrade serveur+client. Pas encore implémenté.

### B.1.5 - Multi-hop HPKE compat (PAS FAIT)

DAITA padding doit s'appliquer **pré-HPKE** côté client (sinon
fingerprinting visible sur la couche externe). Idem côté exit avant le
HPKE inverse direction-tagged. Hook dans `warren-multihop` non encore
fait.

### B.1.6-1.7 - UI Electron + i18n (PAS FAIT)

Composants `WarrenDaitaSwitch.tsx` + `WarrenDaitaSetting.tsx` + gRPC
`WarrenDaitaSettings { enabled: bool }`. Pas démarré.

### B.1.8 - Bench Hetzner overhead (PAS FAIT)

Pas de bench cross-DC DAITA encore. Cible : ≤15% overhead bandwidth vs
baseline 802 Mbps single-hop / 409 Mbps multi-hop M4.E.D.

### B.1.9 - Livré cette session

```
crates/warren-tunnel/src/daita.rs  (567 lignes incl. tests)
  - DaitaConfig          : wire-transmissible spec (machine_specs +
                           max_padding_frac + max_blocking_frac)
  - DaitaEvent           : NormalSent/NormalRecv/PaddingSent/
                           PaddingRecv/TunnelSent/TunnelRecv
  - DaitaAction          : SendPadding/BlockOutgoing/Cancel/UpdateTimer
  - DaitaTimer           : Action/Internal/Both
  - DaitaFramework       : wrap maybenot::Framework<&'static [Machine],
                           ThreadRng>, Debug manual (no leak machine
                           internals), is_enabled/machines_count
  - 10 TDD tests verts
```

Tests :

1. `disabled_config_carries_no_machines`
2. `config_default_is_disabled`
3. `from_config_disabled_yields_disabled_framework`
4. `disabled_framework_returns_no_actions_on_any_event`
5. `from_config_with_noop_machine_builds_enabled_framework`
6. `from_config_with_invalid_machine_string_errors`
7. `from_config_rejects_invalid_padding_fraction`
8. `noop_machine_emits_no_padding_on_normal_events`
9. `simple_netflow_machine_schedules_padding_on_tunnel_sent`
10. `from_machines_round_trip_preserves_count`

Erreurs ajoutées au `TunnelError` :
- `DaitaInvalidMachine(String)` (wire/parsing)
- `DaitaFramework(String)` (framework constructor)

Collateral fix opportuniste : doublon `#[test] #[test]` dans
`allowlist.rs:539-540` (pre-existing duplicate-macro-attribute warning
qui empêchait clippy de passer en strict).

Commit : `ab34ab5 feat(warren-tunnel): wrap maybenot 2.2.2 in
DaitaFramework + DaitaConfig (M5.B.1.1-3 DAITA v2 wire-format
groundwork, server-negotiated machines, 10/10 TDD)`.

---

## B.2 - Multi-exit failover

**PAS DÉMARRÉ**. Scope inchangé du brief.

Surface : `crates/warren-relay-selector/` + `warren-tunnel/src/client.rs`
+ warren-api `/v1/incidents/exit-down` + UI failover toast.

---

## B.3 - Onboarding wizard

**PAS DÉMARRÉ**. Scope inchangé du brief.

Surface : `desktop/packages/mullvad-vpn/src/renderer/` (5 composants
Electron + routing + i18n).

---

## Décisions tactiques actées

1. **DAITA approach** : v2-shaped MVP, server-negotiated machines
   (cf. AskUserQuestion poka 2026-05-20).
2. **Wire format** : *clarifié vs brief* - PROTOCOL_VERSION 2→3 in-band
   (warren-protocol postcard `deny_unknown_fields` + `protocol_version: u8`
   field) **plutôt que** ALPN bump (qui casse M4.0 H3 obfuscation).
   Sélection poka = "bump /v2 majeur" interprétée comme intent breaking
   accepté, mécanisme in-band choisi pour préserver obfuscation.
3. **Rand alias** : `rand_v9 = { package = "rand", version = "0.9" }`
   scopé à warren-tunnel jusqu'à ce que maybenot bump rand 0.10.
4. **Dummy packet format prévu** : first byte 0xFF (non-IP nibble),
   non-breaking pour wire /v1 data plane car les bytes 0x00 et 0xF0-0xFF
   sont déjà invalides comme premier byte IPv4/IPv6.

---

## Critères GO ULTIMATE session B - état

- ❌ B.1 critères GO PASS (~30% : framework wrapper + tests OK, pump +
  handshake + UI + bench non faits)
- ❌ B.2 critères GO PASS (0%)
- ❌ B.3 critères GO PASS (0%)
- ✅ `cargo test -p warren-tunnel --lib daita` 10/10 PASS
- ✅ `cargo clippy -p warren-tunnel --lib --tests -- -D warnings` PASS
- ✅ `cargo fmt --all -- --check` PASS
- ✅ Pas de régression Linux/Mac/Win (modifs locales à warren-tunnel only)
- ✅ Working tree warren-core inchangé sur `d3_allowlist_dynamic.rs`
  (le brief mentionnait ce fichier comme modified-non-committed mais le
  WT était déjà clean au démarrage)
- ✅ Rapport `.planning/session-b-report.md` rédigé

**Verdict** : **GO PARTIEL** - groundwork DAITA v2 prêt-à-construire-dessus,
B.2 + B.3 + suite B.1 reste à faire.

---

## Next steps prioritaires (qui prend la session suivante)

### Tier 1 - terminer B.1

1. **PROTOCOL_VERSION 2 → 3 + DAITA fields** dans warren-protocol
   (`Setup.daita_support: bool`, `SetupAck.daita_spec: Option<DaitaConfig>`).
   Tests : encode/decode round-trip, deny_unknown_fields strict, postcard
   forward-compat semantics documenté.
2. **DaitaDriver async task** dans warren-tunnel (task #13). Hook les 4
   variantes pump (mono, multi, rate-limited). Dummy packet émission +
   filter.
3. **Exit-side machine pool** : 5 machines pré-générées (statiques avec
   seed déterministe pour reproductibilité bench), random pick par
   session, envoi via SetupAck.
4. **Multi-hop HPKE pré-encryption** : hook DaitaDriver dans
   `warren-multihop::session::*` avant le HPKE seal côté client, après
   HPKE unseal côté exit.

### Tier 2 - bench + UI

5. **Bench Hetzner DAITA overhead** cross-DC FR↔FR, 5 min, sustained.
   Cible ≤15%.
6. **UI Electron** : WarrenDaitaSwitch + WarrenDaitaSetting + gRPC +
   i18n FR+EN.

### Tier 3 - B.2 + B.3

7. **B.2 Multi-exit failover** (~1-2 sem) per brief.
8. **B.3 Onboarding wizard** (~3-4j) per brief.

---

## Memory updates à propager

- `warren_daita_groundwork.md` (warren-app + warren-core) : daita.rs
  shipped ab34ab5, framework wrapper sans pump integration, archi
  PROTOCOL_VERSION 2→3 in-band (PAS ALPN /v2 vs sélection préview poka
  - raffinement obfuscation-aware actée doc 20 M4.0).
- Update `warren_phases_roadmap.md` : M5.B.1 DAITA infrastructure
  partielle, suite + B.2 + B.3 pending.
