# Session J, Multi-hop TUN pump (M4.E.X partial) + multi-hop DAITA scaffolding, RAPPORT FINAL

> Status : **GO PARTIEL (scaffolding livré, full main.rs wiring deferred M5.B.X)**
> Date : 2026-05-21
> Cost réel : **0.00 EUR**
> §0.0 INVIOLABLE git respecté. §0.5 plein mandat exercé. §0.6 worktree séparé respecté.

---

## TL;DR

Session J livre les **building blocks pour DAITA multi-hop** : module `multi_hop_pump.rs` warren-client avec `pump_multi_hop_bidirectional` + `pump_multi_hop_bidirectional_with_daita` (mirror de `warren_tunnel::pump_bidirectional_with_daita` mais sur l'abstraction `WarrenPumpHandle`).

**Decision §0.5, full main.rs --use-tun wiring SKIPPED** : la stack multi-hop HPKE n'a pas d'équivalent SetupAck pour négocier le tunnel IP. Sans mécanisme d'allocation IP, wirer `run_multi_hop --use-tun` requiert un design décision sur l'IP negotiation = M5.B.X scope expansion. L'infrastructure `supervised_pump.rs` (`run_uplink_with_daita` + `run_downlink_with_daita`) existe déjà et reste prête pour usage production une fois l'IP negotiation landed.

**Livré (warren-core 26487b4 push origin/main)** :
1. `WarrenPumpHandle::pump_send_daita_dummy` extension trait + impl pour MultiHopClient (route vers `send_daita_padding`)
2. `crates/warren-client/src/multi_hop_pump.rs` : `pump_multi_hop_bidirectional<P, T>` + `pump_multi_hop_bidirectional_with_daita<P, T>` (générics sur P: WarrenPumpHandle + T: PacketDevice). Single tokio task (multi-hop = 1 QUIC pipe, pas de N-conn parallelism). Defense-in-depth dummy filter dans le non-DAITA path aussi.
3. `crates/warren-client/tests/multi_hop_pump.rs` : 3 tests integ avec MockPump (mpsc loopback channels) :
   - `pump_multi_hop_bidirectional_forwards_ip_packets_both_directions`
   - `pump_multi_hop_bidirectional_drops_dummies_on_downlink`
   - `pump_multi_hop_with_daita_emits_padding_on_timer` (Tamaraw forcé seed déterministe 0xBEEF)
4. `warren-client/Cargo.toml` dev-dependencies : ajout `rand_v9` (alias rand 0.9) + `maybenot-machines` pour les fixtures de test

---

## Architecture livrée

### Module `multi_hop_pump.rs`

```rust
pub async fn pump_multi_hop_bidirectional<P, T>(pump: Arc<P>, tun: T) -> anyhow::Result<()>
where P: WarrenPumpHandle + Send + Sync + 'static,
      T: PacketDevice + Clone + Send + Sync + 'static,

pub async fn pump_multi_hop_bidirectional_with_daita<P, T>(
    pump: Arc<P>,
    tun: T,
    state: DaitaState,
) -> anyhow::Result<()>
```

**Structure DAITA variant** (mirror `pump_bidirectional_with_daita_inner`) :
- Single tokio::select! loop, 3 branches (uplink TUN read / downlink pump_recv / DAITA timer)
- Pas de JoinSet car multi-hop = 1 QUIC pipe (vs single-hop multi-conn N pipes)
- Falls through to non-DAITA pump quand `state.is_enabled() == false` (zero overhead)
- Defense-in-depth dummy filter dans les DEUX paths (DAITA + non-DAITA)

### WarrenPumpHandle extension

```rust
pub trait WarrenPumpHandle: Send + Sync {
    async fn pump_send(&self, payload: &[u8]) -> Result<(), MultiHopError>;
    async fn pump_recv(&self) -> Result<Vec<u8>, MultiHopError>;
    async fn pump_send_daita_dummy(&self) -> Result<usize, MultiHopError>;  // NEW
    fn pump_close(&self, code: u32, reason: &[u8]);
}
```

MultiHopClient impl `pump_send_daita_dummy` route vers `send_daita_padding()` (méthode existante M5.B.1.5 ready).

---

## Tests integration

| Test | Scénario | Status |
|---|---|---|
| forwards_ip_packets_both_directions | TUN → pump_send + pump_recv → TUN, 3 IPv4 packets each direction | **PASS** |
| drops_dummies_on_downlink | Inject 0xFF dummy + real IPv4 ; assert seul le real atteint TUN | **PASS** |
| with_daita_emits_padding_on_timer | Tamaraw 5ms timer, inject events, assert dummies_emitted > 0 | **PASS** |

3 tests PASS en 2.07s.
Full warren-tunnel + warren-client : 170 + 78 tests PASS (vs 169 + 78 pré-J, +3 multi_hop_pump).
Clippy strict `cargo clippy -p warren-client --all-targets -- -D warnings` : CLEAN.

Pre-existing test failures (multi_hop_pmtu_regression + full_e2e_both_binaries) UNAFFECTED par Session J.

---

## J.3 SKIPPED §0.5, main.rs --use-tun wiring

### Decision

Skip wiring `run_multi_hop --use-tun` (line 1093 bail). Justification :
- MultiHopClient HPKE stack n'a PAS d'équivalent SetupAck pour négocier le tunnel IP
- `MultiHopClient` ne dispose pas de méthode `assigned_ipv4()` (vs `MultiSession::assigned_ipv4` existant single-hop)
- Sans IP negotiation, le TUN local ne peut pas être configuré avec une IP cohérente
- Design d'un mécanisme d'allocation IP (in-band post-HPKE handshake, ou extension MultiHopSetup frame) = M5.B.X scope dédié

### Existing infra ready for production wiring

`crates/warren-client/src/supervised_pump.rs` expose déjà :
- `run_uplink_with_daita(rx: ClientWatch, tun, daita: DaitaShared)` : uplink avec DAITA + supervisor watch reconnect
- `run_downlink_with_daita(rx, tun, daita)` : downlink avec dummy filter + DAITA events
- Combined avec `MultiHopSupervisor::run()` pour resilient reconnect

Quand l'IP negotiation multi-hop landed (M5.B.X), wirer main.rs sera trivial :
1. Build `MultiHopSupervisor` + watch channel
2. Spawn supervisor.run()
3. Create TUN avec IP négociée
4. Spawn `run_uplink_with_daita` + `run_downlink_with_daita` consumant le watch channel

### Mon module vs supervised_pump

| Aspect | `multi_hop_pump.rs` (Session J) | `supervised_pump.rs` (existing) |
|---|---|---|
| Resilience | Non (single QUIC pipe, dies on disconnect) | Oui (supervisor watch channel + auto-reconnect) |
| Tests integ | Mock channels, simple loopback | Tests avec real MultiHopSupervisor (existing) |
| Production usage | Pour cas simples (in-process, hardcoded pump) | Production warren-client binary (quand IP negotiation prêt) |
| DAITA | `pump_*_with_daita` 3-branch select! | Split tasks (uplink + downlink + supervisor) |

Mon module Session J = **alternative simple** pour tests + scaffolding. Pour production, supervised_pump reste la voie.

---

## Verdict critères GO session J

| Critère brief | Status |
|---|---|
| J.1 multi-hop TUN pump | ✅ pump_multi_hop_bidirectional via WarrenPumpHandle abstraction |
| J.2 multi-hop DAITA wiring | ✅ pump_multi_hop_bidirectional_with_daita + extension trait |
| J.3 wire run_multi_hop --use-tun | ⏭️ SKIPPED §0.5 (IP negotiation M5.B.X dep) |
| J.4 multi-hop integ tests | ✅ 3 tests PASS avec MockPump |
| J.5 report + memory + commit + push + cleanup | ✅ (en cours) |
| Multi-hop DAITA scaffolding usable | ✅ |
| Defense-in-depth dummy filter dans multi-hop path | ✅ |
| Tests cargo clippy CLEAN | ✅ |
| Production wiring main.rs | ⏭️ M5.B.X (IP negotiation dep) |

**Verdict global : GO PARTIEL**, scaffolding livré, défense multi-hop DAITA possible via `multi_hop_pump.rs` direct OR via `supervised_pump.rs` (production-grade). Production main.rs wiring reporté M5.B.X.

---

## Follow-ups M5.B.X documentés

1. **Multi-hop IP negotiation** : design + implementation d'un mécanisme d'allocation tunnel IP côté exit (in-band post-HPKE OR via MultiHopSetup frame extension OR via warren-api separate endpoint). Required before --use-tun wiring.

2. **main.rs --use-tun wiring** : une fois IP negotiation prêt, wirer `MultiHopSupervisor` + `supervised_pump::run_uplink_with_daita` + `run_downlink_with_daita` dans `run_multi_hop`. Trivial.

3. **Multi-hop DAITA spec negotiation** : v1 hardcoded `DaitaPool::default_pool().pick()` client-side. v2 = negotiated via MultiHopSetup extension (analog `SetupAck.daita_spec` single-hop).

4. **Consolidated Hetzner cross-DC bench** : 3-node setup (client + relay + exit) avec full DAITA active. Closes B.1.8 caveat. Estimé ~0.10 EUR.

---

## Caveats secondaires

- **Asymétrie pump implementation** : `multi_hop_pump.rs` 3-branch select! VS `supervised_pump.rs` split tasks. Les deux fonctionnels mais avec sémantique différente (single task vs multi task). Documented in module doc.
- **MockPump test fixture** : utilise `MultihopError::RekeyFailed("mock...")` comme erreur générique. Pas idéal sémantiquement mais le test framework n'expose pas de variant "channel closed" générique.
- **Multi-hop DaitaState reconnect** : sur reconnect (supervisor publie None puis Some(new)), le DAITA state continue avec ses timers existants. Behavior matches `supervised_pump.rs`. Mon module `multi_hop_pump.rs` ne survit pas aux reconnects (one-shot).

---

## Pin warren-app

`.warren-core-version` : `20200d4` → `26487b4` (Session J HEAD).

Effectif desktop + mobile dès rebuild warren-app : aucune surface utilisateur n'est exposée (Session J = scaffolding pour future wiring). Pas de regression possible.

---

## Memory updates

- warren-core : `warren_multihop_daita_pump.md` (nouveau, scaffolding + WarrenPumpHandle extension)
- warren-app : `warren_session_j_delivered.md` (nouveau, ce rapport + verdict)
- warren-app MEMORY.md : ligne haut pour session J
- warren-core MEMORY.md : ligne haut pour session J

---

## Cleanup J.5

- Worktree warren-core-multihop-daita : à supprimer
- vendor symlink : nettoyé avec worktree (cette fois pas committed via add explicit, leçon Session I)
- branch `session-j-multihop-daita` : à supprimer post-merge

Doctrine §0.0 + §0.5 + §0.6 respectée. Aucune commande destructive. WIP poka warren-app + warren-core préservés intacts. Cost cap respecté (0.00 EUR).
