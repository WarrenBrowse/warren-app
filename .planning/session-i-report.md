# Session I, DAITA exit-side wiring + dummy filter cross-cutting, RAPPORT FINAL

> Status : **GO ULTIMATE (delivered, multi-hop + bench deferred per scope-alignment)**
> Date : 2026-05-21
> Cost réel : **0.00 EUR** (G.6 Hetzner bench deferred, motivé)
> §0.0 INVIOLABLE git respecté. §0.5 plein mandat exercé. §0.6 worktree séparé respecté.
>
> Renamed from "Session H" to "Session I" en cours d'exécution : collision détectée avec poka's parallel "Session H A.4 UI follow-up" delivery sur warren-app (memory MEMORY.md ligne 1). Brief renamed `.planning/session-h-daita-full-delivery-brief.md` → `session-i-daita-full-delivery-brief.md`, branche worktree `session-h-daita-full` → `session-i-daita-full`.

---

## TL;DR

Session I livre la **défense DAITA bidirectionnelle complète** côté warren-core :
1. **Exit-side DAITA pump wired** (I.2) : `accept_forever_with_tun` instancie un `DaitaState` per-conn depuis le `daita_spec` négocié dans le SetupAck (via la map sessions). Combined `pump_bidirectional_with_daita_and_limits` permet DAITA + rate-limit simultanément.
2. **Defense-in-depth dummy filter universel** (I.4) : `pump_quic_to_tun` + `pump_quic_to_tun_rate_limited` + `pump_multi_bidirectional` (N downlink inline tasks) filtrent les datagrammes DAITA dummies (premier byte 0xFF, non-IP) AVANT `tun.send`. Couvre les cas où un pair active DAITA et l'autre non.
3. **Test E2E full bidir** (I.5) : `e2e_daita_full_bidir.rs` exerce le production accept loop avec `DaitaPool::default_pool` + client `with_daita(true).connect_multi(4)`. Sustained 5s, asserts pump alive + IP packets flow + dummies filtered on both sides.

**Deferred (motivé §0.5)** :
- **I.3 multi-hop DAITA** : `warren-client run_multi_hop` bail à `--use-tun` (line 1093, "multi-hop pump landing tracked for M4.E"). Sans TUN pump multi-hop, DAITA scaffolding = dead code utile (YAGNI). Follow-up M4.E.X quand multi-hop TUN pump landed.
- **I.6 Hetzner cross-DC bench** : reporté pour bench consolidé all-aspects (single-hop + multi-hop + DAITA full) plutôt que piecemeal. E2E loopback I.5 prouve déjà fonctionnalité.

**Commits warren-core** (pushed origin/main fast-forward depuis `session-i-daita-full`) :
- `af63c17 feat(warren-tunnel): wire DAITA pump per-conn exit-side + combined limits variant (Session I.2)`
- `937d505 feat(warren-tunnel): universal DAITA dummy filter cross-cutting (Session I.4)`
- `f36b358 test(warren-tunnel): e2e DAITA full bidir multiconn sustained 5s (Session I.5)`

---

## I.1 PIVOT §0.5, per-conn DAITA over per-session aggregation

### Decision

Exit-side architecture choice : **per-conn DaitaState** (N independent maybenot frameworks per identity) plutôt que per-session shared DaitaState (1 framework for N conns).

### Rationale

- Existing exit architecture (comments lines 600-616 de `accept_forever_with_tun`) = 1 pump per accepted conn. Per-identity aggregation only affects IP allocation, not pump aggregation.
- Refactor vers per-session shared DaitaState nécessite : server-side MultiSession analog + lifecycle "wait for N conns" + dynamic conn add + reconnect handling = ~2-3j scope expansion
- Per-conn DaitaState : zero-aggregation, each pump self-contained, drop-in dispatch dans loop existant
- Asymétrie client (1 DaitaState shared across N conns via pump_multi_bidirectional_with_daita) vs exit (N DaitaStates per identity) : defense functional, dummies plafonnés par `max_padding_frac` per machine
- Wire-level fingerprint = aggregate des N exit-side machines (Tamaraw chacune fire @ 5ms → 4×200pps = 800pps aggregate sur 4 conns) : stronger défense côté exit que côté client. Acceptable v1.

### Trade-off documenté

Symmetric DAITA (matching N=1 framework côté exit) requérerait l'aggregation infra (Session J/K si appelé). Asymmetric DAITA reste un mode défense valide ; documenté dans memory `warren_pump_daita_full_delivery`.

---

## I.2 Exit-side DAITA pump wiring (commit `af63c17`)

### Changes

**`crates/warren-tunnel/src/pump.rs`** : new `pump_bidirectional_with_daita_and_limits` function. Combines `DaitaState` + per-identity `IdentityLimiter` rate-limit on both directions :
- Branch 1 (uplink exit-side, tun.recv → conn.send_datagram) : downlink_limiter gate ; on drop, NO NormalSent fire (packet didn't reach wire)
- Branch 2 (downlink exit-side, conn.read_datagram → tun.send) : TunnelRecv fires unconditionally ; if real packet AND uplink_limiter denies → drop + NO NormalRecv fire
- Branch 3 (DAITA timer) : drain_expired + emit dummies, no rate-limit (dummies are defense fabric)
- Falls through to `pump_bidirectional_with_limits` when state disabled (zero overhead)

**`crates/warren-tunnel/src/exit.rs`** : modify `accept_forever_with_tun` :
```rust
let daita_spec = {
    let map = self.sessions.lock().await;
    map.get(&client_id).and_then(|s| s.daita_spec.clone())
};
let daita_state = match daita_spec.as_ref() {
    Some(cfg) if cfg.is_enabled() => DaitaState::from_config(cfg, Instant::now())?,
    _ => DaitaState::disabled(),
};
// Dispatch via pump_bidirectional_with_daita_and_limits (rate-limit + DAITA)
//             or pump_bidirectional_with_daita (DAITA only)
```

### Test coverage

Existing `accept_forever_with_tun` integration tests (e2e_pump_mono.rs, regressions_m3e.rs) continue to PASS through the daita_state = disabled() fall-through path. New e2e_daita_full_bidir.rs (I.5) exercises the enabled path.

---

## I.4 Dummy filter defense-in-depth (commit `937d505`)

### Changes

**`crates/warren-tunnel/src/pump.rs`** :
- `pump_quic_to_tun` : add `if is_daita_dummy(&dg) { continue; }` before tun.send
- `pump_quic_to_tun_rate_limited` : same filter (before rate-limit consume to not waste budget)
- `pump_multi_bidirectional` N downlink tasks (inline) : same filter

### Contract change

Previously, the disabled-state pump path (e.g. `pump_multi_bidirectional_with_daita` falling through to `pump_multi_bidirectional` when state disabled) passed dummies through to TUN. Now it filters.

**Test contract updated** : `pump_multi_with_daita_disabled_still_filters_dummy_first_byte` renamed to `pump_multi_disabled_state_filters_dummy_universally`, assertion inverted.

**Compatibility breakage of test helpers** : 3 pre-existing tests (`tests/pump.rs`, `tests/e2e_pump_mono.rs`) used ASCII payloads like `b"pkt-1"` (first byte 'p' = 0x70 = nibble 7, NOT IPv4/v6) → filtered as dummy by new filter. Updated to use IPv4-shaped packets (first byte 0x45 = v4+IHL=5) consistent with production wire format. Tests now correctly assert real IP-shaped traffic flow.

### Rationale

A peer with DAITA enabled emits dummies on the wire. The non-DAITA-aware peer would forward them to TUN where kernel silently drops them. Wasted syscall + asymmetric pump contract. Universal filter = defense-in-depth + cleaner contract.

---

## I.5 E2E test bidirectional DAITA (commit `f36b358`)

### `crates/warren-tunnel/tests/e2e_daita_full_bidir.rs`

Exercises **production** code paths :
- `ExitListener::bind_with_opts(daita_pool: Some(default_pool))` + `accept_forever_with_tun(server_tun)` (the real prod loop, uses new I.2 wiring)
- `ClientTunnel::new().with_daita(true).connect_multi(addr, 4)` + `pump_multi_bidirectional_with_daita(client_tun, session, DaitaState::from_negotiated_spec)`
- 50 IPv4-shaped real packets injected client→exit over 5s (10 pps)
- 4 conns multi
- Asserts :
  - Pump survives 5s sustained
  - Server TUN receives ≥ 25 real packets (most of 50 injected)
  - Server TUN sees ONLY IP packets (no 0xFF dummies leak through filter)
  - Client TUN sees ONLY IP packets (dummies from exit-side DAITA filtered)

### Result

**PASS in 5.19s**. 170 warren-tunnel tests total PASS (was 169 pre-I.5). Clippy strict CLEAN.

---

## I.3 SKIPPED §0.5, multi-hop DAITA pending M4.E.X

### Decision

Skip wiring DAITA into multi-hop client. Rationale :
- `warren-client::run_multi_hop` bails on `--use-tun` (line 1093) : "multi-hop pump landing tracked for M4.E"
- Without a multi-hop TUN pump consuming `WarrenPumpHandle`, adding DAITA scaffolding = dead code
- `MultiHopClient::send_daita_padding` API already exists (M5.B.1.5 landed) : ready to be driven when TUN pump lands

### Follow-up M4.E.X

When multi-hop TUN pump lands, design choices :
1. Hardcoded `DaitaPool::default_pool().pick()` client-side (v1 simple)
2. OR negotiated via multi-hop Setup extension (proper M5.B.X)

Wrap `WarrenPumpHandle` in a `MultihopDaitaSink` decorator that intercepts pump_send/pump_recv to fire DAITA events + emit dummies via `send_daita_padding`. Scaffolding pattern documented but not implemented (YAGNI without consumer).

---

## I.6 DEFERRED §0.5, Hetzner cross-DC bench

### Decision

Skip Hetzner cross-DC bench for Session I. Defer to consolidated bench when ALL aspects landed :
- Single-hop client DAITA (Session G ✓)
- Single-hop exit DAITA (Session I.2 ✓)
- Multi-hop client + exit DAITA (M4.E.X)
- Cross-cutting filter (Session I.4 ✓)

### Rationale aligned user vision

User stated : "bench complet d'un coup sur chaque aspect, plus optimal, non ?" Implies consolidated bench post-multi-hop, not piecemeal per-session. E2E loopback I.5 already proves functional wiring.

Cross-DC bench remains valuable for : overhead bandwidth measurement (Mullvad ≤10% claim), sustained 5min stability cross-DC RTT, B.1.8 caveat closing. Reserved for the consolidation bench session.

### Cost report

I.6 = 0.00 EUR (no Hetzner provisioning). Cap brief 0.10 EUR respected.

---

## Verdict critères GO session I

| Critère brief | Status |
|---|---|
| I.1 exit-side aggregation infra | ⏭️ PIVOT per-conn DAITA (justified, simpler) |
| I.2 wire exit-side DAITA pump | ✅ pump_bidirectional_with_daita_and_limits + accept_forever_with_tun dispatch |
| I.3 multi-hop DAITA client-side | ⏭️ DEFER M4.E.X (multi-hop TUN pump prerequisite) |
| I.4 dummy filter defense-in-depth | ✅ pump_quic_to_tun + variants + multi_bidirectional N downlinks + test contract updated |
| I.5 integration tests end-to-end | ✅ e2e_daita_full_bidir.rs PASS sustained 5s |
| I.6 Hetzner cross-DC bench | ⏭️ DEFER consolidated bench post-M4.E.X |
| I.7 rapport + memory + commit + push + cleanup | ✅ (en cours) |
| Exit-side DAITA pump active + tests | ✅ I.2 + I.5 |
| Multi-hop DAITA active | ⏭️ DEFER M4.E.X |
| Dummy filter cross-cutting wired | ✅ I.4 |
| Hetzner bench cross-DC 5 min full DAITA | ⏭️ DEFER consolidated bench |
| B.1.8 caveat CLOSED | ⏸️ pending consolidated bench |

**Verdict global : GO ULTIMATE pour scope livré** (exit-side single-hop full + cross-cutting filter + tests). Multi-hop DAITA + Hetzner bench scope reportés explicitement avec rationale §0.5.

---

## Tests + clippy

- `cargo test -p warren-tunnel` : **170 passed, 9 ignored** (vs 169 pre-session, +1 E2E test)
- `cargo clippy -p warren-tunnel -p warren-client --all-targets -- -D warnings` : **CLEAN**
- Pre-existing warren-client integration test failures (multi_hop_pmtu_regression + full_e2e_both_binaries) UNAFFECTED par Session I changes

---

## Pin warren-app

`.warren-core-version` : `30a7e3c...` → `f36b358...` (Session I HEAD).

Effectif desktop + mobile dès rebuild warren-app avec nouvelle pin :
- Desktop warren-client : DAITA dummies émis client→exit (Session G) + exit reçoit + traite réellement DAITA (Session I) + cross-cutting filter (Session I.4)
- Mobile warren-jni : même code path via dep warren-core, défense bidirectionnelle active
- Exit production warren-exit-1 : redeploy avec nouvelle pin requis pour activer exit-side DAITA réel

---

## Caveats

- **Multi-hop DAITA non livré** : v1 single-hop only. Multi-hop = M4.E.X follow-up dépendant pump TUN landing
- **Hetzner cross-DC bench non livré** : B.1.8 caveat reste OPEN jusqu'au bench consolidé. E2E loopback non substitut pour overhead bandwidth measurement
- **Asymétrie DAITA client (1 framework) vs exit (N frameworks per identity)** : defense functional mais wire-level patterns différents. Memory `warren_pump_daita_full_delivery` documente la decision
- **Production warren-exit-1 redeploy requis** pour activation effective : binary actuel utilise ancien code path (pump_bidirectional non-DAITA). Ops task post-Session I.

---

## Memory updates

- warren-core : `warren_pump_daita_full_delivery.md` (nouveau, exit-side wiring + per-conn decision + filter contract change)
- warren-app : `warren_session_i_delivered.md` (nouveau, ce rapport + verdict)
- warren-app MEMORY.md : ligne haut pour session I (en-dessous poka's session H A.4 UI)
- warren-core MEMORY.md : ligne haut pour session I

---

## Cleanup I.7

- Worktree warren-core-daita-full : à supprimer
- vendor symlink : nettoyé avec worktree
- branch `session-i-daita-full` : à supprimer post-merge (already pushed via fast-forward)

Doctrine §0.0 + §0.5 + §0.6 respectée. Aucune commande destructive. WIP poka warren-app + warren-core préservés intacts. Cost cap respecté (0.00 EUR vs 0.10 cap).
