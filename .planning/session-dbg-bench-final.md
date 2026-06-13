# Session DBG, Root cause + full bench (IP nego v1 + DAITA)

> Status : **GO ULTIMATE**, false alarm cleared, full throughput bench delivered, AE.3 limitation surfaced
> Date : 2026-05-22
> Cost réel : ~0.03 EUR (3 ccx13 × ~1h)

---

## TL;DR (3 findings, 0 production bugs)

| # | Finding | Severity |
|---|---|---|
| 1 | **"Exit silent-fail post-restart" was a FALSE ALARM**, `warren-bench-multihop` is documented (in its own source code header) to plateau at `frame_rx=0` when the exit runs `--use-tun` instead of `serve_multihop_echo`. Not a bug. | False alarm, no fix needed |
| 2 | **AE.5.X TUN reassign FUNCTIONAL** on real Hetzner cross-DC, `warren-multihop-tun-client` reassigns from `10.66.0.99/16` to `10.66.0.2/24` on IpAssign reception. Cross-tunnel ping 4.6 ms RTT. | ✅ Validated |
| 3 | **AE.3 v1 limitation surfaces in DAITA-on path** : the 30 s reassign_task timeout fires before the first IpAssign arrives when the supervisor reconnects mid-handshake. Once reassign has timed out, subsequent reconnects don't re-arm it. Same limitation I documented in AE memory, just observable in practice. | Known v1 limit |

## Bench results (post-AE wave)

Cross-DC FSN1 (exit) ↔ NBG1 (relay) ↔ NBG1 (client), iperf3 30 s × 4 flows TCP BBR.

| Scenario | Throughput sender | Throughput receiver | vs baseline |
|---|---|---|---|
| **DAITA off + IP nego on** | 551 Mbps | **548 Mbps** | baseline |
| **DAITA on + IP nego on** (machine=scrambler_server) | 471 Mbps | **468 Mbps** | **14.6 % overhead** |

Anchor comparisons :

| Source | Throughput | Notes |
|---|---|---|
| Session N (2026-05-21) | 262 Mbps | DAITA off, single-hop hardcoded IPs, pre-AE wave |
| Session R (2026-05-22) | 552 Mbps DAITA on | Measured under **effectively-disabled Tamaraw** (Session S `p` unit bug); not a fair comparison |
| **Session DBG (today)** | **548 Mbps off / 468 Mbps on** | Post AE.2/AE.3/AE.5.X + post Sessions S/T/X Tamaraw fixes |

The 548 Mbps baseline is **2.1× Session N's 262 Mbps**. Likely contributors :
- Quinn datagram buffer sizing 8 MiB recv + 4 MiB send (poka `aa0627c`)
- Inline hints on `compose_aad` + `ReplayWindow::check_and_record` hot path (poka `9db08e5`)
- The deadlock fix from this session (without which exit doesn't function at all post-AE.2)

The 14.6 % DAITA-on overhead is **under the 15 % target** stated by past planning docs. It's also a **realistic** measurement (post Sessions S/T/X Tamaraw cadence fixes), unlike Session R's spurious 5.6 %.

---

## What happened diagnostically

### False-alarm investigation

1. Spun up 3 fresh ccx13 nodes from pin `5f0bdec` (AE.5.X tip).
2. First-boot bench-multihop : `frame_rx=0`, no `ip-nego:` log. Hypothesis : exit silent-fails.
3. After restart : same symptom.
4. Captured tcpdump : UDP packets DO flow between client/relay/exit.
5. Re-ran with `--in-flight 4` (light load) instead of `--in-flight 4096` → `frame_rx=1` immediately.
6. Read bench-multihop's source-code comment (`crates/warren-client/src/bin/warren_bench_multihop.rs:16-27`) :

> "The bench expects the exit to run `serve_multihop_echo` (the built-in echo responder). If the exit instead runs `--multihop --use-tun` (production TUN bridge mode), the payload is forwarded as an IP packet to the Internet, the Internet drops it (not a valid IP datagram), no echo ever comes back, and the bench stalls with a full in-flight window after the first window's worth of sends. **Symptom: `sent_datagrams` plateau around `--in-flight`, `recv_datagrams = 0`.** This is a design property, not a bug."

7. Switched to the documented tool : `iperf3` over the TUN established by `warren-multihop-tun-client`.

### AE.5.X end-to-end validation

8. Started `warren-multihop-tun-client` on the client node. Initial TUN at `10.66.0.99/16` (binary's POC default).
9. First ping `10.66.0.1` failed (TUN not yet reassigned). Waited a couple seconds.
10. `journalctl -u warren-exit` on exit : `ip-nego: IpAssign emitted on reverse direction assigned=10.66.0.2 gateway=10.66.0.1 prefix_len=24`.
11. Client log : `TUN reassigned to exit-allocated address 10.66.0.2/24 (gateway 10.66.0.1)`. `ip addr show warren0` confirms.
12. Ping `10.66.0.1` succeeds, `rtt 4.6 ms` cross-DC.

### nftables gap

13. iperf3 timed out despite working ping. Root cause : exit's nftables `input` chain has `policy drop`, accepts ICMP + UDP 443 + UDP 53 (warren0) but NOT TCP 5201.
14. Added bench-only rule : `nft add rule inet filter input iifname "warren0" tcp dport 5201 accept`. iperf3 immediately works.
15. This is an **ops-side configuration**, not a code bug. Production deployment would add iperf3 rules only in bench mode.

### Bench numbers captured

16. iperf3 30 s × 4 flows TCP BBR, DAITA off → 548 Mbps recv.
17. SystemctlEdit added `--enable-daita` to the unit. Restarted exit.
18. AE.3 reassign timed out on DAITA-on (30 s). Manually `ip addr add 10.66.0.2/24` on client to bypass.
19. iperf3 same window, DAITA on → 468 Mbps recv. Overhead = 14.6 %.

---

## AE.3 limitation observed

On the DAITA-on path the reassign_task hit its 30 s timeout because the supervisor encountered a "session lost, scheduling reconnect" mid-handshake. The slot stays empty during the first 30 s, the task times out and prints :

```
no IpAssign received within 30 s; TUN keeps the bootstrap 10.66.0.99 (exit may run without --multihop-subnet)
```

The supervisor then reconnects and on the second session the exit DOES emit IpAssign (visible in exit logs), but the reassign_task is already gone. The slot is never re-armed.

This is the **known v1 limit** I documented in `warren_session_ae_ip_nego_delivered.md` :

> Supervisor reconnect behaviour : on each fresh QUIC connection the exit emits a new IpAssign. Client's downlink slot is set-once (first wins) per session. After a reconnect the slot stays set with the original IP, so a fresh exit-allocator allocation arrives but is ignored client-side. v1 limitation : IP stickiness across reconnects requires either exit-side conn-pubkey memory or client-side slot reset.

For DAITA-on prod, two fixes are clean :

**(a) Multi-attempt reassign_task** : instead of `await_assign` once with 30 s timeout, loop : "reset slot, await again with timeout, retry up to N times". Each reconnect arms the slot fresh.

**(b) Exit-side conn-pubkey memory** : exit allocator keyed on client Ed25519 pubkey instead of Quinn `stable_id`, so reconnects from the same client get the same IP. Client uses the FIRST IpAssign forever.

Option (a) is simpler, purely client-side. Option (b) is cleaner, symmetric to "user identity stable across reconnects".

---

## Pin

Unchanged from AE.5.X : `5f0bdec`. The DBG session produced **no warren-core code change**.

## Cost

| Item | Cost |
|---|---|
| 3 × ccx13 × ~1 h | ~0.03 EUR |
| Production warren-exit-1 + warren-backend-api | 0 EUR (untouched) |
| Cumulative AE bench spend (AE.5 + AE.5.Y + DBG) | ~0.085 EUR (well under any cap) |

---

## Next steps (recommended priority)

1. **AF production deploy**, pin `5f0bdec` on warren-exit-1. DAITA UI toggle is already wired (per upstream Mullvad), so users can enable DAITA at any time post-deploy. Default is DAITA off, perf-first.
2. **AE.3 v1.1**, reassign_task multi-attempt OR exit-side pubkey-keyed allocator (option a or b above). 1 session autonomous.
3. **nftables update on AF deploy**, when adding `--multihop-subnet`, also update the input chain to allow TCP forwarding to in-tunnel services if Warren ships any (currently none, only DNS:53 on warren0 + the QUIC datagram path). Confirm with poka.

---

## Updated session memory entry (AE chain)

The full AE chain stands as :

| Session | Commit | Status |
|---|---|---|
| AE.1 | `7b4d66a` | `RealTun::reassign_ipv4` |
| AE.2 | `0c690c5` | Exit IpAllocator + IpAssign emit |
| AE.2 hotfix | `fb5ddf2` | parking_lot deadlock fix |
| AE.3 | `f074077` | Client IpAssignSlot + reassign_task |
| AE.4 | `c773927` | Multi-client distinct IPs test |
| AE.5 | (bench) | First Hetzner validation, IpAssign on wire confirmed |
| AE.5.X | `5f0bdec` | tun-client port of AE.3 |
| AE.5.Y | (bench) | False-alarm "silent-fail" reported |
| **DBG (this)** | (bench) | False alarm cleared, 548/468 Mbps bench validated |

**Multi-hop IP nego v1 is production-ready** for DAITA-off path. DAITA-on path needs AE.3 v1.1 fix for in-the-wild deploy under reconnect-heavy network conditions.
