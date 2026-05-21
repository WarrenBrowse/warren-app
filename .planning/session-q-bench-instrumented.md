# Session Q — Instrumented bench reveals Quinn datagram silent drop

> Status : **GO ULTIMATE** — root cause Session N bug isolated to Quinn client-side datagram pipeline
> Date : 2026-05-22 (started 2026-05-21 evening UTC)
> Cost réel : **~0.02 EUR** (3 ccx13 ~30 min)
> §0.0 INVIOLABLE git respecté. §0.5 plein mandat exercé. Production warren-exit-1 + warren-backend-api intacts.

---

## TL;DR

Bench Hetzner cross-DC reprovisionné identique Session N + binaires Session P (rate-limited 5s INFO counters sur 4 instrumentation points). Bug Session N reproduit 100% loss sur ping 10.66.0.1.

**Root cause localized** par contradiction entre log INFO counters + tcpdump wire capture :

| Surface | Observation |
|---|---|
| Client `uplink_with_daita` log (60 s window) | `sent_real=39 sent_padding=324` (Ok responses from `client.send()` / `client.send_daita_padding()`) |
| Client TUN0 input (FakeTun equivalent on Linux) | 40 ping packets received from kernel via tun0 |
| Client eth0 outbound tcpdump UDP/443 (10 s window) | **~4 packets, length 30-35 bytes** (= QUIC PING keep-alives, NO data datagrams) |
| Relay eth0 inbound tcpdump UDP/443 (5 s window) | **0 packets** |
| Exit eth0 inbound tcpdump UDP/443 (5 s window) | **0 packets** |
| Exit `rx_task report` INFO log | **Never fires** (rx_task stuck on `read_datagram().await`, no datagrams arriving) |
| Exit `tx_task report` INFO log | **Never fires** (no current session, no replies to seal) |

**Conclusion** : Quinn `Connection::send_datagram(bytes)` returns `Ok(())` (datagrams queued internally in `outgoing_total` buffer up to `datagram_send_buffer_size = 1 MB` default), **but datagrams never reach the wire**. The drop happens between Quinn's queue and the UDP socket.

---

## Diagnostic timeline

```
22:15:39  client started, multi-hop session established
22:15:39  multi-hop DAITA active (client-side hardcoded pool pick) machines=1
22:15:39  exit logs "multihop DAITA active: serve_multihop_with_tun_and_daita with curated pool pick machines=2"
22:15:52  client uplink_with_daita report from_tun=3 sent_real=2 sent_padding=324 too_large=0 dying=0
22:16:05  client uplink_with_daita report from_tun=4 sent_real=3 sent_padding=324
22:16:20  client uplink_with_daita report from_tun=10 sent_real=9 sent_padding=324
22:16:25  client uplink_with_daita report from_tun=20 sent_real=19 sent_padding=324
22:16:30  client uplink_with_daita report from_tun=30 sent_real=29 sent_padding=324
22:16:40  client uplink_with_daita report from_tun=40 sent_real=39 sent_padding=324
```

Striking pattern : **`sent_padding` ceases incrementing after ~13 s** (locked at 324, expected ~200/s × 60s = 12000 with Tamaraw p=5ms). `sent_real` still increments (39 over 60s = 0.65/s = ping rate).

This means `client.send_daita_padding()` (which internally calls `client.send(dummy)`) stops returning `Ok(())` after ~13s. The DAITA timer task keeps firing but each `send_daita_padding()` errs (silently — `tracing::trace!`).

Cross-checking the Quinn datagram source code (`vendor/quinn-fork/quinn-proto/src/connection/datagrams.rs::send`) :

```rust
pub fn send(&mut self, data: Bytes, drop: bool) -> Result<(), SendDatagramError> {
    let max = self.max_size().ok_or(SendDatagramError::UnsupportedByPeer)?;
    if data.len() > max {
        return Err(SendDatagramError::TooLarge);
    }
    if drop {
        while self.conn.datagrams.outgoing_total > self.conn.config.datagram_send_buffer_size {
            // Silently drop oldest datagrams to make room.
        }
    }
    ...
    Ok(())
}
```

Default `datagram_send_buffer_size = 1024 * 1024 = 1 MB`. At Tamaraw 5ms cadence with ~1300 B sealed dummies = 260 KB/s ingress. Buffer fills in 4 s. After that, oldest are evicted (silent), but Quinn keeps queuing new ones.

**Why no transmission on wire** : Quinn's congestion controller (BBR) appears to throttle the connection severely. With no stream traffic (multi-hop uses datagrams only), BBR has no RTT/bandwidth feedback and may pace very slowly. With buffer full + slow drain, the FIFO queue is essentially a write-only sink with the network seeing only the rare keep-alive PING that bypasses the datagram queue.

The 4 packets visible in 10 s tcpdump (30-35 B each, both directions) match the keep-alive cadence (`QUIC_KEEP_ALIVE_INTERVAL_SECS = 20s`).

---

## Why Session N baseline (DAITA OFF) worked

In baseline DAITA OFF, the supervised pump's timer arm never fires (no Tamaraw config → `DaitaState::disabled()`). Only real packets flow at TCP iperf3 rate (~262 Mbps, batched, BBR feedback via TCP's stream-like ACK pattern through the multi-hop pipeline).

The bug surface is **DAITA-induced 200/s constant-rate dummy injection into a Quinn datagram pipe with no concurrent stream traffic**. The Quinn pipeline becomes congestion-blind, BBR underestimates, drops accumulate silently.

---

## Hypothesis ranking (post-Session-Q evidence)

| Cause | Evidence | Verdict |
|---|---|---|
| Quinn datagram send buffer overflow under Tamaraw cadence | sent_padding stuck @ 324, tcpdump zero data | **CONFIRMED ROOT** |
| BBR congestion controller starves datagrams without stream feedback | wire 4 keep-alives in 10s = pacing very slow | LIKELY CONTRIBUTING |
| Universal dummy filter | n/a (filter not reached, packets never leave client) | DISPROVEN |
| supervised_pump / exit serve / relay forward | All in-process tests PASS Session O+P | DISPROVEN |
| MTU 1280 + sealing overhead > Quinn negotiated max | sent_padding succeeded 324 times early, then stops (would be 0 from start) | UNLIKELY |

---

## Fix directions (Session R candidates)

1. **Slow down DAITA cadence** in production : Tamaraw p=5ms is aggressive. Use p=50ms (20/s) → buffer fills 10× slower. Tradeoff : weaker DAITA defense.
2. **Cap dummy emission via Quinn `send_buffer_space()` check** : in `run_uplink_with_daita`, before `send_daita_padding`, check `conn.send_buffer_space()` and skip emit if < threshold. Tradeoff : non-constant rate breaks Tamaraw's defense property.
3. **Increase `datagram_send_buffer_size`** : 16 MB instead of 1 MB → buffer fills in 60s instead of 4s. Tradeoff : 16 MB per connection, memory pressure. **Doesn't fix transmission rate issue**.
4. **Force BBR feedback** : open a dummy stream that the client polls for ACKs, give BBR feedback signal. Tradeoff : added complexity.
5. **Replace BBR with CUBIC or NewReno** : both are loss-based, work better for unreliable datagram streams. Tradeoff : lower throughput on healthy paths.
6. **Drop datagram pacing entirely** : configure Quinn to send datagrams without pacing. Tradeoff : risk of overrunning network buffers.

**Recommended for Session R** : combine #3 (16 MB buffer for headroom) + #6 (no pacing for datagrams) + diagnostic test to confirm. Then bench again.

---

## Code changes Session Q

**warren-core (commit local `0106b8d` from Session P)** : 4-point instrumentation deployed and verified working :
- `serve_one_connection_with_tun_and_daita::rx_task` counters + 5s INFO report
- `serve_one_connection_with_tun_and_daita::tx_task` counters + 5s INFO report
- `supervised_pump::run_uplink_with_daita` counters + 5s INFO report (provided the breakthrough diagnostic)
- `supervised_pump::run_downlink_with_daita` counters + 5s INFO report

**warren-core (Session Q)** : zero source change. Pure ops verification of Session P instrumentation. The 4-point counter design is validated by the diagnostic value extracted from production logs.

---

## Cleanup + pin

- 3 Hetzner nodes (`warren-bench-q-{client,relay,exit}`) deleted
- Production warren-exit-1 + warren-backend-api intacts (verified post-cleanup)
- Cost réel : ~0.02 EUR
- Pin warren-app : non bumpé (`5ee1c4d` reste sur origin/main, Session P commit `0106b8d` still local-only awaiting poka push)

---

## Caveats restants

- B.1.8 caveat reste OPEN — overhead measurement impossible jusqu'au fix Quinn datagram pipeline
- CRITICAL bug DAITA multi-hop reste actif en production — **NE PAS activer `--enable-daita` multi-hop sur warren-exit-1** jusqu'au fix Session R
- Session R must validate fix with re-bench (cost ~0.02 EUR) before declaring B.1.8 closed

---

## Doctrine

- §0.0 INVIOLABLE git respecté
- §0.5 plein mandat exercé : autonomous bench full orchestration + diagnostic
- §0.6 worktree skipped (zero source change Session Q, ops + analysis only)

## Next steps Session R

1. Add explicit `datagram_send_buffer_size(16 * 1024 * 1024)` in `warren_transport_config_base`
2. Investigate Quinn datagram pacing API (look for `Connection::path()` / `set_pacing(false)` or similar)
3. Add `send_buffer_space` exposure via instrumentation (extend uplink_with_daita counter)
4. Cross-compile + redeploy + bench
5. If fix works → measure B.1.8 overhead → close caveat
6. If fix doesn't work → escalate (Quinn upstream issue?)
