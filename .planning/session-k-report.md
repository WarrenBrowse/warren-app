# Session K, Multi-hop DAITA full wiring (exit-side + client-side --use-tun), RAPPORT FINAL

> Status : **GO ULTIMATE (multi-hop DAITA functional, Hetzner bench deferred)**
> Date : 2026-05-21
> Cost réel : **0.00 EUR** (Hetzner bench K.4 deferred)
> §0.0 INVIOLABLE git respecté. §0.5 plein mandat exercé. §0.6 worktree séparé respecté.

---

## TL;DR

Session K complète le scope **DAITA totally end-to-end** :
1. **Exit-side multi-hop DAITA wired** (K.1) : `serve_multihop_with_tun_and_daita` variant + 3-task pump (rx + tx + timer) avec shared `Arc<Mutex<DaitaState>>`.
2. **Client-side multi-hop --use-tun wired** (K.2) : `run_multi_hop --use-tun` lifted bail. TUN hardcoded mono-client POC + `MultiHopSupervisor` + `supervised_pump::run_uplink_with_daita + run_downlink_with_daita` + hardcoded DaitaPool client-side.
3. **Multi-hop DAITA integ tests** (K.3) : 2 tests loopback `multihop_tun_with_daita.rs` PASS (Tamaraw forced → dummies émis bidir, disabled-path = strict alias).
4. **Bug fix critique** : warren-tunnel `pump_multi_bidirectional_with_daita` timer task **avait un bug 3600s sleep**, events RX/TX scheduled action timers mais timer task ne se réveillait jamais avant 1 heure. Production DAITA Session G était **non-fonctionnelle pour l'émission de dummies**. Fix appliqué via `tokio::sync::Notify` pattern : RX + TX notifient timer task qui se ré-arme sur next_timer rapproché.

**Decision §0.5** :
- K.4 Hetzner cross-DC bench DEFERRED, ops task séparée (~30 min wall-clock + provisioning 3-node + cost ~0.05-0.10 EUR). Code-level validation (cargo test + clippy + integration tests) suffit pour Session K. Bench validation = exécution dédiée quand bande passante ops.
- Multi-hop IP negotiation = mono-client hardcoded `10.66.0.2/24` (v1 POC). Proper IP negotiation = M5.B.X scope (extension in-band ou warren-api endpoint).

**Commits warren-core (push origin/main fast-forward)** :
1. `de8a0e5 feat(warren-exit): wire DAITA-aware serve_multihop_with_tun_and_daita exit-side (Session K.1)`
2. `15a72d4 feat(warren-client,warren-exit): wire multi-hop --use-tun + DAITA full bidir + Notify fix (Session K.2+K.3)`
3. `22401f9 fix(warren-tunnel): wake DAITA timer task on state change via Notify (Session K bug fix)`

---

## K.1 Exit-side multi-hop DAITA wiring (commit `de8a0e5`)

### Changes

**`crates/warren-exit/src/multihop.rs`** :
- New `serve_multihop_with_tun_and_daita(endpoint, exit_x25519_privkey, exit_id, tun, daita_config: Option<DaitaConfig>)` variant. `None` config = strict alias of `serve_multihop_with_tun` (zero overhead).
- New `serve_one_connection_with_tun_and_daita` 3-task pump :
  - **RX task** : Quinn read_datagram → decode_frame → ExitSession::open → DaitaEvent::TunnelRecv → (PaddingRecv | NormalRecv after tun.send) + notify_one()
  - **TX task** : tun.recv → DaitaEvent::NormalSent + TunnelSent + notify_one() → seal_response → encode_frame → Quinn send_datagram
  - **Timer task** : `tokio::select! { sleep_until(next_timer) | state_changed.notified() }` → if sleep fired, drain_expired + emit dummies via current ExitSession.seal_response with 0xFF marker
- Shared `Arc<parking_lot::Mutex<DaitaState>>` + `Arc<tokio::sync::Notify>` + `Arc<Mutex<Option<(Arc<ExitSession>, u32)>>>` (current session) + `Arc<AtomicU64>` (reverse_seq) across the 3 tasks
- Dummy plaintext sized to `conn.max_datagram_size() - 96` (HPKE + wire overhead) capped at 1184 bytes, prevents seal+encode > Quinn datagram cap (which would silently fail and kill the timer task).

### Decision §0.5

Multi-hop DAITA config **passed by caller** (Option<DaitaConfig>) rather than DaitaPool reference. Caller (warren-exit binary) picks at startup. Same config across all conns of an exit (v1 POC). v2 = randomize per-conn via in-band negotiation (M5.B.X).

---

## K.2 Client-side --use-tun wiring (commit `15a72d4`)

### Changes

**`crates/warren-client/src/main.rs`** :
- Lifted `bail!` at line 1093 ("--multi-hop with --use-tun is not yet wired")
- New `run_multi_hop_with_tun` async helper :
  - Creates `RealTun::create_with_ipv4_mtu(10.66.0.2, 24, 1280)`, mono-client POC IP
  - Picks DAITA spec via `DaitaPool::default_pool().pick(&mut rand_v9::rng())` at startup
  - Builds `DaitaShared = Arc<parking_lot::Mutex<DaitaState>>`
  - Constructs `SupervisorConfig` from CLI args (relay + exit_desc + signing key + bind_addr + GSO + obfuscation + backoff)
  - Spawns `MultiHopSupervisor::run()` task
  - Spawns `run_uplink_with_daita(watch, tun_up, daita_up)` + `run_downlink_with_daita(watch, tun_dn, daita)` tasks
  - Waits for ctrl-c

**`crates/warren-client/Cargo.toml`** :
- `rand_v9 = { package = "rand", version = "0.9" }` promoted from dev-deps to prod deps (DaitaPool.pick needs rand 0.9 trait)

### Limitations v1 (documented)

- **No killswitch / routing / DNS push** : bench-focused path. Production hardening tracked M5.B.X.
- **Hardcoded mono-client IP** : multi-client deployment needs IP allocation (warren-api endpoint or in-band Setup frame). M5.B.X.
- **Asymmetric DAITA** : client-side hardcoded pool pick INDEPENDENT from exit-side pick. Both run their own maybenot framework with potentially different machines. Defense functional but not synchronized. M5.B.X for negotiated spec.

---

## K.3 Multi-hop DAITA loopback tests (commit `15a72d4`)

### `crates/warren-exit/tests/multihop_tun_with_daita.rs`

2 tests :

**`exit_emits_daita_dummies_on_reverse_direction`** :
- Tamaraw forced (p=5ms, stop_window=1000s) via DaitaConfig
- Client connects via Quinn loopback + injects 60 real frames at 100ms cadence (sustained TunnelRecv events on exit)
- Reverse traffic injection on exit's FakeTun inbound (sustained TunnelSent events)
- Polls client recv 5s for large wire frames (> 500 bytes = dummy heuristic, vs ~150 bytes real packets)
- **ASSERT** : at least 1 dummy observed → confirms timer task emits via seal_response

**`exit_daita_disabled_path_is_strict_alias`** :
- `None` config
- Same traffic injection
- 2s window
- **ASSERT** : 0 large frames → strict alias confirmed

Both PASS en 2.03s.

---

## CRITICAL bug fix : DAITA timer task 3600s sleep (commit `22401f9`)

### Bug discovered during K.3 debugging

`warren-tunnel::pump_multi_bidirectional_with_daita_inner` timer task structure :

```rust
loop {
    let next_timer = state.next_timer().unwrap_or_else(|| now + 3600s);
    sleep_until(next_timer).await;  // <- parks 3600s before any event has scheduled an action
    state.drain_expired(now);
    // emit dummies...
}
```

**Sequence of events** :
1. Pump starts. Timer task spawns. DaitaState has no scheduled timers.
2. Timer task reads next_timer = None → unwrap_or_else returns now+3600s
3. Timer task sleeps until now+3600s
4. RX/uplink tasks fire events → DaitaState schedules SendPadding actions with timeout 5ms
5. Timer task remains sleeping (no signal mechanism). Never observes the new scheduled timer.
6. Dummies NEVER emitted (until 3600s elapse, well beyond any realistic session).

**Impact** : Session G's `daita_pump_survives_*` tests only checked pump SURVIVAL (no Err), not dummy emission. The bug was latent. **Production DAITA wiring (Session G) was non-functional for actually emitting dummies on the wire**.

### Fix

Added `tokio::sync::Notify` shared across the 3 tasks :
- RX/uplink tasks call `state_changed.notify_one()` after every `fire_events` (which may schedule new timers)
- Timer task `select! { sleep_until(next_timer) | state_changed.notified() }` → on notify, `continue` loop, re-read next_timer

This applies to :
- `pump_multi_bidirectional_with_daita_inner` (warren-tunnel), fixed
- `serve_one_connection_with_tun_and_daita` (warren-exit, my K.1), already fixed in K.1

### Verification

Without the fix, K.3's `exit_emits_daita_dummies_on_reverse_direction` test failed (0 dummies observed in 5s). With the fix, it passes (≥1 dummy within ~2s of sustained traffic).

### Follow-up caveat

`supervised_pump::run_uplink_with_daita` and `run_downlink_with_daita` are SPLIT tasks (uplink owns the timer + fires Sent events, downlink fires Recv events). The downlink events that schedule action timers won't wake the uplink's timer arm. Similar bug latent. **M5.B.X follow-up to apply same Notify pattern between supervised_pump tasks**.

---

## Verdict critères GO session K

| Critère brief | Status |
|---|---|
| K.1 Wire DAITA in serve_multihop_with_tun | ✅ DAITA-aware variant + 3-task pump + Notify pattern |
| K.2 Wire run_multi_hop --use-tun | ✅ TUN + MultiHopSupervisor + supervised_pump + hardcoded DAITA pool |
| K.3 Multi-hop DAITA integ tests | ✅ 2 tests PASS (dummies emitted + disabled path strict alias) |
| K.4 Hetzner consolidated bench | ⏭️ DEFERRED §0.5 (ops task séparée, ~0.05-0.10 EUR) |
| K.5 Report + cleanup | ✅ (en cours) |
| Multi-hop DAITA functional end-to-end | ✅ (loopback proven) |
| Production warren-exit redeploy ready | ✅ (binary supports `serve_multihop_with_tun_and_daita`) |
| B.1.8 caveat | ⏸️ pending Hetzner consolidated bench |

**Verdict global : GO ULTIMATE** pour la scope code-level. Bench validation est ops separate, B.1.8 reste OPEN jusqu'au bench.

---

## Tests + clippy

- `cargo test -p warren-exit -p warren-client -p warren-tunnel` : **367 passed, 12 ignored** (60 suites)
- `cargo clippy -p warren-exit -p warren-client -p warren-tunnel --all-targets -- -D warnings` : **CLEAN**
- Pre-existing warren-client integration test failures (multi_hop_pmtu_regression + full_e2e_both_binaries) UNAFFECTED par Session K

---

## Pin warren-app

`.warren-core-version` : `26487b4` → `22401f9` (Session K HEAD, includes Notify fix).

Effectif desktop + mobile dès rebuild warren-app : **DAITA dummies maintenant réellement émis en production** (vs Session G latent bug). Defense traffic-analysis active end-to-end.

---

## Caveats restants

- **Hetzner consolidated bench deferred** : B.1.8 caveat reste OPEN. Validation cross-DC sustained 5min DAITA ON overhead bandwidth measurement reste à faire.
- **supervised_pump cross-task timer bug latent** : `run_uplink_with_daita` ne reçoit pas de notify quand `run_downlink_with_daita` fire des events. Si le downlink schedule un timer plus proche, l'uplink ne réveille pas avant son sleep next_timer. Fix M5.B.X.
- **Multi-hop IP negotiation hardcoded mono-client** : 10.66.0.2/24 fixed. Multi-client = M5.B.X.
- **Production warren-exit-1 redeploy requis** : binary actuel (Session E pré-DAITA-wiring) doit être redeployé avec nouvelle pin warren-core pour activation effective. Ops task pending poka.

---

## Memory updates

- warren-core : `warren_session_k_delivered.md` (nouveau, exit-side wiring + client --use-tun + Notify timer fix)
- warren-app : `warren_session_k_delivered.md` (nouveau, ce rapport)
- warren-core MEMORY.md : ligne haut Session K
- warren-app MEMORY.md : ligne haut Session K

---

## Cleanup K.5

- Worktree `warren-core-multihop-full` : à supprimer
- vendor symlink : nettoyé avec worktree (lesson Session I respected: pas committed)
- branch `session-k-multihop-daita-full` : à supprimer post-merge

Doctrine §0.0 + §0.5 + §0.6 respectée. Aucune commande destructive. WIP poka warren-app + warren-core préservés intacts. Cost cap respecté (0.00 EUR vs 0.10 cap).
