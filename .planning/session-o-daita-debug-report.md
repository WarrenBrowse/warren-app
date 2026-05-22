# Session O — DAITA multi-hop debug + regression gates

> Status : **GO PARTIEL** — regression gates added, root cause Session N bug not isolated
> Date : 2026-05-21
> Cost réel : **0 EUR** (in-process only, zero Hetzner spend)
> §0.0 INVIOLABLE git respecté. §0.5 plein mandat exercé. §0.6 worktree skipped (no parallel work, ops cleanup-only Session N pin then test-add).

---

## TL;DR

Session N empirical bench surfaced a critical bug: DAITA-on multi-hop `--use-tun` produces "tunnel established" but **0 throughput**. Session O attempted in-process reproduction across two angles:

1. **Exit side** (`multihop_tun_with_daita_data_flow.rs`) : runs `serve_multihop_with_tun_and_daita` against a fake client + Tamaraw config + sealed real packets. **PASSES** — exit forwards real client uplink packets to its TUN.
2. **Client side** (`pump_with_supervisor_daita.rs`) : runs full MultiHopSupervisor + `run_uplink_with_daita` + `run_downlink_with_daita` against a fake relay-as-exit + Tamaraw config + real IPv4-shaped payloads. **PASSES** — supervised pump round-trips real packets.

**Both new regression gates pass in-process.** Real bench failure must originate from **system-integration** (relay + exit + Linux TUN + DAITA combined), not from any of the isolated subsystems above.

Bonus debugging discovery: the dummy filter (`is_daita_dummy` from Session I.4) correctly distinguishes IPv4 (0x4N) / IPv6 (0x6N) first nibbles. An earlier draft of the client-side test used `XOR_MASK = 0x5A` which mangled `0x45 → 0x1F` (nibble 1), causing all echoed packets to be filtered. This was a test artifact, NOT the bench bug.

---

## Tests added

### `crates/warren-exit/tests/multihop_tun_with_daita_data_flow.rs`

Regression gate against future bugs in `serve_one_connection_with_tun_and_daita` data-plane:

- Spawns exit QUIC endpoint with Tamaraw config (`stop_window = 1e9` to keep machine active)
- Client seals 40 real IPv4-shaped payloads (first byte 0x45, recognizable `0xCAFEBABE` tag at bytes 2-5) and sends them via QUIC datagrams every 50 ms
- Polls `FakeTun.take_outbound()` for matching frames
- **Asserts ≥ 1 real packet surfaces at exit's PacketDevice** within 2 s

Pre-existing K.3 test (`multihop_tun_with_daita.rs`) only validated dummy emission. This new gate validates the orthogonal property (real-data forwarding), which Session N revealed was the failing surface in production.

### `crates/warren-client/tests/pump_with_supervisor_daita.rs`

Regression gate against future bugs in `supervised_pump::run_*_with_daita`:

- Spawns fake relay-as-exit (echo responder, identity mask) + warren-relay-style HPKE flow
- Builds full `MultiHopSupervisor` + spawns `run_uplink_with_daita` + `run_downlink_with_daita` against shared `DaitaState` (Tamaraw) + `Notify` cross-task pattern (Session L)
- Injects 10 real IPv4-shaped packets via FakeTun inbound
- Polls outbound for echoed payloads
- **Asserts ≥ 1 round-trip succeeds** within 5 s

This is the FIRST end-to-end client-side DAITA pump test. Catches deadlocks, filter regressions, timer starvation, supervisor disconnect races.

---

## Pourquoi le bug Session N reste non-isolé

The bench symptom = 100% packet loss bidirectionnal under DAITA. The hypotheses:

| Hypothesis | Verdict |
|---|---|
| Universal dummy filter drops IPv4 packets (first nibble = 4 ≠ dummy) | **DISPROVEN** — filter logic correct, IPv4 always nibble 4 |
| Client supervised_pump uplink starves real packets vs DAITA timer | **DISPROVEN** in-process — round-trip works |
| Exit serve_multihop_with_tun_and_daita drops real packets when DAITA on | **DISPROVEN** in-process — real packets reach FakeTun |
| Relay forwards DAITA dummies different than baseline | **NOT TESTED** in-process (would need 3-process pipeline + DAITA) |
| Real Linux TUN behaves differently than FakeTun under DAITA cadence | **NOT TESTED** — requires Linux host |
| Quinn `max_datagram_size` shrinks for relay→exit hop under sustained dummy stream | **NOT TESTED** — needs production trace |
| Kernel rp_filter / nft drops ICMP reply path on exit warren0 | **NOT TESTED** — needs production trace |
| QUIC connection state degrades under high-rate DAITA dummies (BBR estimator confused?) | **NOT TESTED** — needs production trace |

**Conclusion** : the bug is in the **system-integration** layer (multi-hop relay + DAITA dummies + real Linux TUN), not in any subsystem tested in isolation.

---

## Next steps (Session P+)

1. **Production instrumentation** : add `warn!` / `info!` logs (rate-limited) on critical paths:
   - `supervised_pump::run_downlink_with_daita`: log on `is_daita_dummy(payload)` filter hits + on `tun.send` success/failure
   - `serve_one_connection_with_tun_and_daita::rx_task`: log on `session.open(&frame)` failures (currently silent `continue`)
   - `serve_one_connection_with_tun_and_daita::tx_task`: log on `tun.recv` success (proves kernel delivers ICMP replies)
   - Relay `pump_one_direction`: count `dropped_too_large` + log if non-zero
2. **3-process in-process test** : extend `multi_hop_e2e.rs` pattern with real `RelayServer` + real `serve_multihop_with_tun_and_daita` (no XOR responder, just IP-echo) + Tamaraw config. If THIS fails, bug isolated to relay+exit-DAITA combo.
3. **Re-bench Hetzner** with new TRACE-level logs → diagnose which `continue` swallows the packets.

---

## Code livré

- `crates/warren-exit/tests/multihop_tun_with_daita_data_flow.rs` (+208 LOC)
- `crates/warren-client/tests/pump_with_supervisor_daita.rs` (+232 LOC)
- Commit warren-core `5ee1c4d` push origin/main, `cargo test` PASS + `cargo clippy --release` strict CLEAN
- Pin warren-app `8e7c042` → `5ee1c4d`

---

## Caveats restants

- **B.1.8 caveat reste OPEN** (overhead bench impossible jusqu'au fix DAITA multi-hop bug)
- **CRITICAL BUG** Session N multi-hop DAITA reste actif en production (NE PAS activer `--enable-daita` en mode multi-hop sur warren-exit-1)
- In-process tests pass = test coverage gates against regressions, NOT proof of production correctness

---

## Doctrine

- **§0.0 INVIOLABLE** : zero destructive git, eprintln debug instrumentations cleanly reverted before commit
- **§0.5 plein mandat** : autonomous in-process debug exécuté, abort §0.5 retenu when production bench bug not in-process reproducible (avoid another Hetzner spend without instrumentation strategy)
- **§0.6 worktree** : skipped justified (zero cross-repo parallel work, test-only additions)

## Cost récap

- **0 EUR** (in-process only, zero Hetzner spend this session)
- Production warren-exit-1 + warren-backend-api préservés intacts
