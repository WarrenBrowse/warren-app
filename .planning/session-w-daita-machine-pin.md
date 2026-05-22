# Session W — `--daita-machine` CLI flag for deterministic pool pick

> Status : **GO ULTIMATE** — bench / ops debug surface added
> Date : 2026-05-22
> Cost réel : **0 EUR** (in-process only)
> §0.0 INVIOLABLE git respecté. Production warren-exit-1 + warren-backend-api intacts.

---

## TL;DR

Random pool pick is correct production behaviour (defense diversity) but a pain for bench / ops :
- Bench wants to measure a specific defense's overhead, not roll a different machine each run
- Ops debug wants reproducibility across binary restarts

Session W adds `--daita-machine <name>` to both `warren-exit` and `warren-client` (multi-hop --use-tun path). When set with `--enable-daita`, overrides the random `pick_with_name(rng)` call with the deterministic `pick_named(name, rng)` API added in Session U. Unknown name → loud error listing valid entries.

---

## Code livré (commit warren-core `4444420`)

### `crates/warren-exit/src/main.rs`
- New `--daita-machine <name>` CLI arg, `requires = "enable_daita"` so it can't be set without DAITA being active.
- Multi-hop pool pick site now branches on `args.daita_machine.as_deref()` :
  - `Some(name)` → `pool.pick_named(name, &mut rng)`; errors loudly listing `pool.entry_names()` on unknown name.
  - `None` → existing `pool.pick_with_name(&mut rng)` (production default).
- INFO log gains `pinned = args.daita_machine.is_some()` so the operator can confirm whether the pin took effect.

### `crates/warren-client/src/main.rs::run_multi_hop_with_tun`
- Same `--daita-machine` arg, mirrored doc (notes that single-hop path receives its spec from `SetupAck` and does not benefit from this client-side pin).
- Same pick branching logic.
- INFO log gains `pinned` field.

---

## Usage examples

```bash
# Force Tamaraw on the exit (deterministic bench).
warren-exit --multihop --enable-daita --daita-machine tamaraw ...

# Force the client's multi-hop --use-tun path to use Scrambler.
warren-client --multi-hop --use-tun --enable-daita --daita-machine scrambler_server ...

# Random pool pick (production default).
warren-exit --multihop --enable-daita ...

# Unknown name surfaces all valid entries at startup.
warren-exit --multihop --enable-daita --daita-machine bogus ...
# → Error: --daita-machine 'bogus' not found; valid entries:
#   ["netflow", "tamaraw", "front", "interspace_server", "scrambler_server"]
```

---

## Validation

| Test | Status |
|---|---|
| `cargo check -p warren-exit -p warren-client` | OK |
| `cargo clippy --release -p warren-tunnel -p warren-exit -p warren-client --tests -- -D warnings` | CLEAN |
| `daita_pool_pick_cadence` (Session S/T) | 2/2 PASS |
| `daita_pool_full_coverage` (Session U) | 2/2 PASS |
| `multi_hop_e2e_with_daita` (Session P) | 1/1 PASS |
| `pump_with_supervisor_daita` (Session O) | 1/1 PASS |

---

## Pin warren-app

`.warren-core-version` : `f0be037` (Session V) → `4444420` (Session W).

---

## Doctrine

- §0.0 INVIOLABLE git respecté
- §0.5 plein mandat exercé : targeted CLI surface for ops + bench, no architectural over-extension
- §0.6 worktree skipped (single targeted change)

## Next steps (hors bench)

1. **Pump-side blocking enforcement** : queue real packets during `BlockOutgoing` window (full Tamaraw "block + pad" defense property — `bypass: true` currently lets real packets through alongside padding). Architecture decision required : product tradeoff between defense quality and throughput ceiling (Tamaraw caps to ~200 pkt/s → ~2 Mbps).
2. **Multi-hop IP negotiation v1 multi-client** : replace POC `10.66.0.2/24` hardcoded in `run_multi_hop_with_tun`. Requires HPKE-sealed Setup/SetupAck-equivalent over the multi-hop datagram channel.
3. **DAITA UI/docs announce** : surface defense status (active machine, padding emit counters) in desktop UI + landing page.
