# Session X, DaitaMetrics per-session counters

> Status : **GO ULTIMATE**, production observability surface
> Date : 2026-05-22
> Cost réel : **0 EUR** (in-process only)

---

## TL;DR

Per-session counters on `DaitaState` so production loggers + in-process regression tests can query the actual DAITA emission profile without scraping every per-task counter snapshot.

Three counters tracked :
- `padding_fired`, `SendPadding`-kind action timers drained (= dummies the pump caller emits)
- `blocking_begins`, `BlockOutgoing`-kind action timers drained (= state-1 → state-2 transitions for Tamaraw / Scrambler)
- `blocking_ends`, `block_end_at` instants elapsed (= `BlockingEnd` events fired back into the framework)

---

## Code livré (commit warren-core `d1d5297`)

### `crates/warren-tunnel/src/daita.rs`
- New `pub struct DaitaMetrics { padding_fired, blocking_begins, blocking_ends }`. `Copy + Default + PartialEq + Eq + Debug + Clone`.
- `DaitaState` carries a `metrics: DaitaMetrics` field (initialised in `from_config` + `disabled`).
- `pub fn metrics(&self) -> DaitaMetrics` returns a snapshot.
- `drain_expired` increments `padding_fired` / `blocking_begins` (kind-dispatched) and `blocking_ends` (block_end_at elapsed).
- Uses `saturating_add` to avoid overflow on long-lived sessions.

### `crates/warren-tunnel/tests/daita_pool_pick_cadence.rs::tamaraw_fires_padding_at_expected_cadence`
- Extended assertion : Tamaraw must fire exactly one `BlockingBegin` (state 1 → 2 transition), zero `BlockingEnd` (MAX_SAMPLED_BLOCK_DURATION = ~1 day far beyond the 1 s window), and `padding_fired + blocking_begins == fired_count` (the local drain loop count).

---

## Validation

| Test | Status |
|---|---|
| `daita_pool_pick_cadence::tamaraw_fires_padding_at_expected_cadence` | **PASS** (metrics assertions added) |
| `daita_pool_pick_cadence::pool_pick_with_name_returns_consistent_entry` | PASS unchanged |
| `daita_pool_full_coverage` (Session U) | 2/2 PASS unchanged |
| `cargo clippy --release -p warren-tunnel --tests -- -D warnings` | CLEAN |

---

## Pin warren-app

`.warren-core-version` : `4444420` (Session W) → `d1d5297` (Session X).

---

## Doctrine

- §0.0 INVIOLABLE git respecté
- §0.5 plein mandat exercé : targeted observability addition, no over-extension into pump-side semantics
- §0.6 worktree skipped (single targeted change)

## Next steps (hors bench)

1. **Pump-side blocking enforcement** : significant scope, architectural decision required (Tamaraw caps to ~200 pkt/s = ~2 Mbps).
2. **Multi-hop IP negotiation v1 multi-client** : replace POC `10.66.0.2/24` in `run_multi_hop_with_tun`. Requires HPKE-sealed Setup/SetupAck-equivalent over multi-hop datagram channel.
3. **Surface DaitaMetrics in production logs** : log snapshot on session close (multi-hop client + exit) for ops visibility. Small follow-up to Session X.
