# Session U, Default pool coverage gate + non-blocking machine audit

> Status : **GO ULTIMATE (in-process)**, Session T's BlockingBegin/End wiring confirmed coverage for both blocking pool entries (Tamaraw + Scrambler); the three non-blocking entries audited and proven not to need additional event wiring
> Date : 2026-05-22
> Cost réel : **0 EUR** (in-process only, zero Hetzner spend per user request "hors bench")
> §0.0 INVIOLABLE git respecté. Production warren-exit-1 + warren-backend-api intacts.

---

## TL;DR

Session T closed Tamaraw's BlockingBegin wiring gap. Session U audits the rest of the curated default pool to confirm no other entries need similar fixes :

- **Audit result** : `SimpleNetFlow` (`netflow`), `Front` (`front`), `InterspaceServer` (`interspace_server`) source files use only `Action::SendPadding` plus events Warren's `DaitaState` already wires (`Tunnel*`, `Normal*`, `Padding*`). The framework-fired events they consume (`LimitReached`, `CounterZero`) are emitted internally by maybenot without Warren-side wiring.
- **Scrambler** (`scrambler_server`) does use `Action::BlockOutgoing` + `Event::BlockingBegin`, so Session T's wiring path is exercised by Scrambler too.

Added coverage tests :
1. `all_default_pool_entries_build_a_valid_config`, sanity check that every named entry materialises a non-empty `DaitaConfig` (5/5).
2. `scrambler_server_fires_padding_through_blocking_begin_wiring`, 5 s simulated trace proves Scrambler emits ≥ 1 padding action through Session T's `BlockingBegin/End` machinery. If Session T regresses, this fires alongside the existing Tamaraw cadence test.

`netflow` + `interspace_server` are intentionally NOT asserted strictly (they have legitimate slow / stimulus-dependent cadences : NetFlow 1.5-9.5 s padding interval, Interspace requires sustained burst patterns) and instead rely on the `multi_hop_e2e_with_daita` / `pump_with_supervisor_daita` end-to-end pump tests for behavioural validation.

---

## Code livré (commit warren-core `067d21c`)

### `crates/warren-tunnel/src/daita_pool.rs`
- New `pub fn pick_named<R>(&self, name: &str, rng: &mut R) -> Option<DaitaConfig>`, deterministic per-entry config builder (the random `pick` path is unsuitable for per-entry regression tests).

### `crates/warren-tunnel/tests/daita_pool_full_coverage.rs` (NEW, +120 LOC)
- `drive_and_count` helper : runs a 1 ms drain cadence with `NormalSent + TunnelSent` events every 50 ms, returns total padding actions drained.
- `all_default_pool_entries_build_a_valid_config`, iterates every `entry_names()` entry, builds via `pick_named`, asserts `is_enabled() && !machine_specs.is_empty()`.
- `scrambler_server_fires_padding_through_blocking_begin_wiring`, pins Scrambler specifically, asserts ≥ 1 padding in 5 s of simulated traffic.

---

## Audit results detail

| Entry | Actions | Events transitioning state | Warren-DaitaState wired? |
|---|---|---|---|
| `netflow` (SimpleNetFlow) | `SendPadding` only | `TunnelSent / TunnelRecv` | ✅ all events wired pre-T |
| `tamaraw` | `BlockOutgoing` + `SendPadding` | `NormalSent`, `BlockingBegin`, `TunnelSent` | ✅ Session T |
| `front` (FRONT) | `SendPadding` only (200-state machine, per-state limit) | `PaddingSent`, `LimitReached` (framework-fired), `NormalSent`, `NormalRecv` | ✅ all caller-fired events wired |
| `interspace_server` | `SendPadding` only | `Normal*`, `Padding*`, `Tunnel*` | ✅ all events wired |
| `scrambler_server` | `BlockOutgoing` + `SendPadding` | `Normal*`, `Padding*`, `BlockingBegin`, `LimitReached` | ✅ Session T |

The framework-fired events `LimitReached` and `CounterZero` are emitted internally by maybenot when a state's `limit` / `counter` distribution hits zero, they don't require Warren-side wiring.

---

## Validation post Session U

| Test | Status |
|---|---|
| `daita_pool_full_coverage::all_default_pool_entries_build_a_valid_config` | **PASS** |
| `daita_pool_full_coverage::scrambler_server_fires_padding_through_blocking_begin_wiring` | **PASS** |
| `daita_pool_pick_cadence::tamaraw_fires_padding_at_expected_cadence` (Session S/T) | PASS |
| `daita_pool_pick_cadence::pool_pick_with_name_returns_consistent_entry` | PASS |
| `pump_with_supervisor_daita` (Session O) | PASS unchanged |
| `multi_hop_e2e_with_daita` (Session P) | PASS unchanged |
| `cargo clippy --release -p warren-tunnel --tests -- -D warnings` | CLEAN |

---

## Pin warren-app

`.warren-core-version` : `5d0e8a1` (Session T) → `067d21c` (Session U).

---

## Caveats restants

- ⚠️ Pump-side blocking enforcement still pending, Warren emits real packets during the BlockOutgoing window (Tamaraw/Scrambler cadence correct via `bypass: true` but full "block + pad" defense property requires pump-side queuing of real packets, a separate scope).
- ⚠️ `daita_sustained_stress` + `d3_allowlist_dynamic` Quinn handshake test flakiness pre-existing (`StatelessRetryIssued`), not Session U scope.
- ⚠️ Hetzner re-bench to remeasure overhead under functional Tamaraw cadence (post Sessions S/T/U fix stack) deferred per user "hors bench" instruction. Code ready for the bench whenever ops cycle is scheduled.

---

## Doctrine

- §0.0 INVIOLABLE git respecté
- §0.5 plein mandat exercé : audit + targeted coverage test, no over-extension into pump-side enforcement (separate scope)
- §0.6 worktree skipped (single targeted source + test addition)

## Next steps (non-bench-only)

1. **Pump-side blocking enforcement** : queue real packets during `BlockOutgoing` window so the full Tamaraw "block + pad" defense property is achieved (currently `bypass: true` lets real packets through alongside padding, defeating the on-wire indistinguishability).
2. **Fix Quinn handshake test flakiness** in `daita_sustained_stress.rs` + `d3_allowlist_dynamic.rs` (pre-existing, blocks full DAITA suite green).
3. **Multi-hop IP negotiation v1 multi-client** (replace POC `10.66.0.2/24` in `run_multi_hop_with_tun`).
4. **DAITA UI/docs announce** : with Tamaraw/Scrambler now functional, surface the defense status in the desktop UI + landing page.
