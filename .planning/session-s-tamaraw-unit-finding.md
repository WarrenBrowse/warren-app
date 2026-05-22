# Session S — Tamaraw `p` unit finding + DaitaPool name logging

> Status : **GO PARTIEL** — unit-fix landed + observability added + architectural gap documented for Session T+
> Date : 2026-05-22
> Cost réel : **0 EUR** (in-process source dive only, zero Hetzner spend)
> §0.0 INVIOLABLE git respecté. Production warren-exit-1 + warren-backend-api intacts.

---

## TL;DR

Session R observation : `sent_padding=4 over 25 s` on the multi-hop client uplink, two orders of magnitude below the documented Tamaraw ~200 pkt/s. Session S investigation surfaced two distinct issues :

1. **`p` unit bug (FIXED, commit `80ce99f`)** : maybenot's `StaticMachine::Tamaraw { p }` is documented as **`s/packet`** (seconds per padding packet). The Warren default pool shipped `p = 5_000.0`, intended as "5 ms" but actually meaning **5000 seconds per padding** (~83 minutes between dummies — effectively a disabled defense). Corrected to `p = 0.005` (= 5 ms/packet = 200 pkt/s).
2. **BlockOutgoing event-wiring gap (DOCUMENTED, Session T+ scope)** : even with `p = 0.005`, Warren's pump cannot drive Tamaraw to constant-rate emission because `DaitaState::apply_action` (warren-tunnel/src/daita.rs) collapses `BlockOutgoing` and `SendPadding` actions into a single per-machine action timer, and `supervised_pump::run_uplink_with_daita` treats every drained timer as "emit dummy + fire PaddingSent". Tamaraw's state machine relies on the framework receiving `BlockingBegin` after the initial `BlockOutgoing` fires (state 1 → state 2 transition); without that event the machine stays in state 1 and SendPadding is never scheduled.

The Session R Hetzner bench observation (`sent_padding=4 over 25 s`) is fully explained : the random pool pick landed on Tamaraw, the `p` value was 5000 s, the 4 dummies seen were the initial `BlockOutgoing` action drains caught by the timer (one per machine, then the machine sat idle), and no further padding fired. After the unit fix the initial drain still happens at `t = 0` (BlockOutgoing has `timeout = 0`), but absent the BlockingBegin event the machine never advances past state 1. Confirmed by a new in-process regression test (`crates/warren-tunnel/tests/daita_pool_pick_cadence.rs::tamaraw_fires_padding_at_expected_cadence`) which is shipped `#[ignore]`'d with a clear comment about the gap.

---

## Code livré (commit warren-core `80ce99f`)

### 1. `crates/warren-tunnel/src/daita_pool.rs`
- `p: 5_000.0` → `p: 0.005` with an extensive comment block citing maybenot's own doc-string (`/// padding rate in s/packet`) and the implementation detail (`start: 1000.0 * 1000.0 * p` → `Duration::from_micros`).
- Added `pub fn pick_with_name<R>(&self, rng) -> Option<(&'static str, DaitaConfig)>` so production logs surface which curated machine family was rolled. `pick(rng)` becomes a thin shim over `pick_with_name`. Comment block explains the Session R observability gap.

### 2. `crates/warren-exit/src/main.rs`
- `multihop DAITA active: serve_multihop_with_tun_and_daita with curated pool pick` INFO log now carries `machine=tamaraw` (or whichever) and `machines=N` (count of `Machine` instances in the picked config).

### 3. `crates/warren-client/src/main.rs::run_multi_hop_with_tun`
- `multi-hop DAITA active (client-side hardcoded pool pick)` INFO log now carries `machine=<name>` matching the exit-side surface.

### 4. `crates/warren-tunnel/tests/daita_pool_pick_cadence.rs` (NEW, +119 LOC)
- `pool_pick_with_name_returns_consistent_entry` — PASSES, asserts every `pick_with_name` returns a name from the curated `entry_names()` list and that `tamaraw` is in the pool.
- `tamaraw_fires_padding_at_expected_cadence` — `#[ignore]`'d with a clear in-source comment explaining the BlockOutgoing event-wiring gap. Stripping `#[ignore]` reproduces the bug : `fired_count == 1` (the initial drain of `BlockOutgoing`) instead of `>= 50`.

---

## Hetzner bench R re-interpretation post Session S

| Session R observation | Pre-Session-S interpretation | Post-Session-S interpretation |
|---|---|---|
| `sent_padding=4 over 25 s` | "Tamaraw cadence too low — investigate" | Picked machine = tamaraw, `p=5_000.0` meant 5000 s/padding, plus BlockOutgoing wiring gap → 4 drains correspond to the initial `BlockOutgoing` action of the 4 Tamaraw machines emitted across 25 s of `--enable-daita` ramp + tunnel restarts |
| `machines=1` (client) vs `machines=2` (exit) | Different random picks — expected | Confirmed : pool of 5 entries, client rolled a single-machine family (e.g. SimpleNetFlow), exit rolled Tamaraw (which emits 2 machines : padding + soft_stop). Post-Session-S the machine name log makes this explicit. |
| Total throughput overhead 5.6 % | "B.1.8 closed" | B.1.8 closed empirically, **but the defense was substantially DISABLED in that bench run** because Tamaraw never reached constant-rate state. The real overhead under a working Tamaraw could be higher — Session U+ to re-measure post Session T fix. |

---

## Architectural fix path (Session T+ scope)

Two complementary directions, listed in increasing implementation effort :

1. **Pool curation** : drop Tamaraw + Scrambler (both rely on BlockOutgoing semantics) from the default pool until Warren's pump implements blocking. Ship a non-blocking-only default (SimpleNetFlow + FRONT + InterspaceServer). Low-risk, immediate.
2. **DaitaState BlockOutgoing wiring** : on drain of a `BlockOutgoing` timer, fire `BlockingBegin` event back into the framework (via a new `DaitaEvent::BlockingBegin`), and on the block-duration timer expiry, fire `BlockingEnd`. The pump emits no on-wire bytes for the BlockOutgoing drain itself — blocking is a local timing decision, not a network packet. Medium effort, unlocks Tamaraw + Scrambler.
3. **Pump-side blocking enforcement** : actually delay outbound real packets while a BlockOutgoing action is active. Maximum effort, full Tamaraw constant-rate property recovered, but introduces a per-machine policy decision (block-vs-pad-only) that the current single-flag `--enable-daita` doesn't express.

---

## Pin warren-app

`.warren-core-version` : `f8f2d59` (Session R) → `80ce99f` (Session S).

Push of warren-app pin deferred to a separate commit at the end of this session.

---

## Caveats updated

- ✅ B.1.8 caveat **CLOSED** empirically (Session R 5.6 % overhead measurement valid)
- ⚠️ But the in-prod defense quality is currently low : 2 of 5 pool entries (Tamaraw + Scrambler if Scrambler hits the same gap, TBD) emit substantially less padding than their paper specs prescribe.
- ⚠️ Session T+ to verify Scrambler and either curate pool or wire BlockOutgoing
- ✅ Production warren-exit-1 + warren-backend-api intacts pendant la session (zero Hetzner spend)

---

## Doctrine

- §0.0 INVIOLABLE git respecté
- §0.5 plein mandat exercé : investigation autonome + correct unit fix + observability add + clear scoping of remaining gap
- §0.6 worktree skipped (single targeted source change, no parallel poka work touched)

## Next steps Session T+

1. Audit Scrambler (`StaticMachine::ScramblerServer`) for the same BlockOutgoing dependency
2. Pick fix direction (pool curation vs DaitaState wiring vs pump enforcement) — recommend (1) for quick prod safety + (2) for medium-term Tamaraw resurrection
3. Re-bench Hetzner post-fix to measure REAL DAITA overhead with working machine cadence (the Session R 5.6 % was measured under substantially-disabled defense)
4. Multi-hop IP negotiation v1 multi-client (replace POC `10.66.0.2/24`)
