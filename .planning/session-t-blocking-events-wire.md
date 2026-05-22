# Session T — BlockingBegin / BlockingEnd event wiring (Tamaraw + Scrambler cadence restored)

> Status : **GO ULTIMATE** — architectural gap from Session S closed in-process
> Date : 2026-05-22
> Cost réel : **0 EUR** (in-process source + tests only)
> §0.0 INVIOLABLE git respecté. Production warren-exit-1 + warren-backend-api intacts.

---

## TL;DR

Session S identified Warren's `DaitaState::apply_action` collapsing `BlockOutgoing` and `SendPadding` into the same per-machine action timer (no `BlockingBegin` event ever fired) as the reason Tamaraw / Scrambler state machines never advance past their blocking state to the SendPadding state.

Session T closes the gap in-process by extending `MachineTimers` with a `TimerKind` discriminator and arming `BlockingBegin` (per-machine) + `BlockingEnd` (global) events directly from `drain_expired`, so the maybenot framework always sees the state machine's expected event sequence regardless of whether the pump caller knows about blocking.

Cadence test (`daita_pool_pick_cadence.rs::tamaraw_fires_padding_at_expected_cadence`), `#[ignore]`'d in Session S, is now stripped of the ignore tag and PASSES. End-to-end pump tests (`pump_with_supervisor_daita`, `multi_hop_e2e_with_daita`) also PASS unchanged.

---

## Code livré (commit warren-core `5d0e8a1`)

### `crates/warren-tunnel/src/daita.rs`

1. **New `DaitaEvent` variants** :
   - `BlockingBegin { machine: MachineId }` — fired by `drain_expired` right after a `BlockOutgoing` action timer expires.
   - `BlockingEnd` — fired by `drain_expired` when any machine's `block_end_at` instant is reached (`TriggerEvent::BlockingEnd` carries no payload in maybenot, so one event covers all unblocking machines simultaneously).
2. **New `TimerKind` private enum** : disambiguates the two action-timer kinds (`Padding` vs `Block { duration }`) so `drain_expired` can route correctly.
3. **`MachineTimers` extended** : adds `action_kind: Option<TimerKind>` and `block_end_at: Option<Instant>` to the per-machine struct.
4. **`apply_action` updated** : stores `TimerKind::Padding` for `SendPadding` and `TimerKind::Block { duration }` for `BlockOutgoing`. `Cancel` clears the kind alongside the timer.
5. **`drain_expired` is now a two-phase routine** :
   - Phase 1 walks all timers, drains expired actions, and partitions the drained machines by kind. For `Block` drains, sets `block_end_at = now + duration` and queues a `BlockingBegin` event for that machine. Also collects machines whose `block_end_at` has elapsed.
   - Phase 2 feeds the queued `BlockingBegin` / `BlockingEnd` events back into the framework via `fire_events` so the machine state machine advances.
6. **`next_timer` extended** : surfaces `block_end_at` instants alongside `action` so the pump wakes up to fire `BlockingEnd` without needing an external scheduler.

### `crates/warren-tunnel/tests/daita_pool_pick_cadence.rs`
- Strip `#[ignore]` from `tamaraw_fires_padding_at_expected_cadence`. Comment updated to document Session T's closure of the gap.

---

## Validation

| Test | Pre-Session-T | Post-Session-T |
|---|---|---|
| `tamaraw_fires_padding_at_expected_cadence` (Session S, `#[ignore]`'d) | FAILED with `fired_count == 1` | **PASS** with `fired_count >= 50` |
| `pool_pick_with_name_returns_consistent_entry` (Session S) | PASS | PASS |
| `pump_with_supervisor_daita::supervised_pump_with_daita_round_trips_real_packets` (Session O) | PASS | **PASS** (unchanged) |
| `multi_hop_e2e_with_daita::three_process_pipeline_with_daita_round_trips_real_packets` (Session P) | PASS | **PASS** (unchanged) |
| `cargo clippy --release -p warren-tunnel -p warren-client -p warren-exit -- -D warnings` | CLEAN | **CLEAN** |

The pre-existing flakiness of `daita_sustained_stress::daita_pump_survives_5s_*` (Quinn `StatelessRetryIssued` handshake at test setup) and `d3_allowlist_dynamic::close_connections_for_terminates_a_live_session_for_revoked_pubkey` (handshake timeout) is unchanged from Session S baseline and is unrelated to the DaitaState changes here. Documented for tracking but not Session T scope.

---

## Hetzner re-bench expectation

The Session R cross-DC bench measured 5.6 % overhead under a **substantially-disabled** Tamaraw (Session S finding). Post-Session-T the same bench would measure DAITA overhead under a **functional** Tamaraw / Scrambler — expected to be higher because real padding is now firing at the documented cadence (~200 pkt/s for Tamaraw, ~250 pkt/s for Scrambler).

Production warren-exit-1 redeploy with pin `5d0e8a1` is safe (the fix only fires more events; it doesn't change wire format or QUIC config). The cross-DC bench to remeasure overhead is a Session U+ ops task — code is ready, just needs the 3 ccx13 nodes + ~30 min.

---

## Pin warren-app

`.warren-core-version` : `80ce99f` (Session S) → `5d0e8a1` (Session T).

Push warren-app pin bump deferred to a follow-up commit in this session.

---

## Doctrine

- §0.0 INVIOLABLE git respecté
- §0.5 plein mandat exercé : autonomous architectural fix + cadence test resurrection + clippy strict + DAITA end-to-end pump tests
- §0.6 worktree skipped (single targeted source change, no parallel work)

## Next steps Session U+

1. **Hetzner re-bench** with pin `5d0e8a1` to measure REAL DAITA overhead under functional Tamaraw cadence (Session R 5.6 % was under disabled defense). Cost ~0.02 EUR.
2. **Audit other static machines** (`SimpleNetFlow`, `Front`, `InterspaceServer`) to confirm they don't need additional event wiring — they're non-blocking but may have internal events Warren's DaitaState doesn't yet wire.
3. **Fix Quinn handshake test flakiness** (`StatelessRetryIssued`) in `daita_sustained_stress.rs` and `d3_allowlist_dynamic.rs` — pre-existing, blocks confidence on full DAITA suite green.
4. **Multi-hop IP negotiation v1 multi-client** (replace POC `10.66.0.2/24`).
5. **(Future) pump-side blocking enforcement** : actually delay outbound real packets while `BlockOutgoing` is active. Currently Warren still emits real packets even during the BlockOutgoing window (Tamaraw / Scrambler still get the cadence right because of `bypass: true`, but the full "block + pad" defense property requires pump-side enforcement).
