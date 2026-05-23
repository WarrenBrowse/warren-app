# Session AE.5.Y — Throughput bench retry (post AE.5.X port)

> Status : **STOPPED — new production issue surfaced, AE.5.X validation deferred**
> Date : 2026-05-22
> Cost réel : ~0.025 EUR (3 ccx13 × ~45 min)

---

## TL;DR

AE.5.X tun-client port committed and pushed to origin/main (`5f0bdec`). Bench retry uncovered **a new production issue separate from the AE.2 deadlock** : after a `systemctl restart warren-exit`, fresh QUIC connections from clients reach the exit's UDP socket but **the exit's `serve_one_connection_with_tun_and_daita` rx_task never processes any frame** — no `ip-nego: IpAssign emitted` log, no rx_task report, zero reverse-direction frames. Both `warren-bench-multihop` and `warren-multihop-tun-client` produce the same symptom on the fresh-restart exit.

The earlier successful IpAssign emit at 10:51:04 (Session AE.5) happened on the **first-boot** state of the exit binary, after the deadlock fix had landed and the binary started cleanly. After SIGTERM-killed-by-stop-timeout + restart, the exit accepts incoming UDP (tcpdump confirms 181 B sealed frames arrive) but never reaches the application's `session.open()` path.

Pragmatic decision : **stop the bench, tear down the 3 nodes (~0.025 EUR), document the finding** for a focused debug session.

---

## What was validated

| Item | Status |
|---|---|
| AE.5.X — port `IpAssignSlot` + `reassign_task` into `warren-multihop-tun-client` POC bin | ✅ committed `5f0bdec`, pushed to origin/main |
| cargo check + clippy + DAITA tests | ✅ all pass |
| Cross-build of `warren-exit` + `warren-client` + `warren-relay` from pin `5f0bdec` | ✅ via cross-rs Docker (2m49 total) |
| 3-node Hetzner provision (exit FSN1 + relay/client NBG1) | ✅ all 3 up, mnemonic-derived identities, signed descriptors |
| Exit startup with `multihop IP allocator ready capacity=253` | ✅ AE.2 deadlock fix from `fb5ddf2` confirmed live |

## What didn't work

| Item | Symptom |
|---|---|
| `warren-bench-multihop` against fresh-restart exit | `frame_rx_datagram_total: 0` (expected 1 IpAssign) |
| `warren-multihop-tun-client` reassign task | times out at 30 s with "no IpAssign received" |
| ping cross-tunnel | 100 % packet loss |
| Exit journalctl post-restart | **silent** — zero `ip-nego:` / rx_task / decode_errs / open_errs logs despite incoming UDP confirmed by tcpdump |

## What we know

- **UDP layer is healthy** : `tcpdump -i eth0 udp port 443` on the exit shows client→relay→exit 181 B sealed frames + 37 B exit→relay→client QUIC ACKs flowing both directions.
- **QUIC handshake completes** : the 37 B response packets are QUIC ACKs from the exit's Quinn server, so the Quinn accept side is functional.
- **Application-layer frame processing fails silently** : the `serve_multihop_with_tun_and_daita` outer `endpoint.accept().await` either doesn't fire OR fires but the spawned per-conn task never reaches `session.open()`.
- **First-boot of fresh exit IS working** : the same binary on the same Hetzner box did emit `IpAssign assigned=10.66.0.2` on the bench-multihop connection at 10:51:04 (Session AE.5, before the systemctl restart cycle).
- **Same binary post-restart breaks** : both `bench-multihop` and `tun-client` consistently fail to trigger IpAssign or any per-conn log.

## Most likely root causes (untested hypotheses)

1. **Quinn endpoint state leaks across the restart**. The `bind_addr 138.199.236.149:443` is reused. Some kernel-level UDP socket state, port reuse, or systemd LingerOnRestart=yes could be confusing Quinn's accept loop. The first-boot acceptance worked because the port was fresh; post-restart it's dirty.

2. **A `SIGTERM stop-sigterm timed out → SIGKILL` cycle leaves the Quinn accept loop in a non-functional state**. The journal shows `Killing process 2433 (tokio-rt-worker) with signal SIGKILL` — a hard kill in the middle of the accept loop. If the new binary inherits something corrupt (e.g., a UDP socket bound to the same port without proper SO_REUSEPORT), the accept-loop returns futures that never resolve.

3. **The TUN device `warren0` is in a bad state post-restart** : earlier tcpdump on `warren0` showed zero packets despite tunnel traffic. Maybe the new exit binary's `RealTun::create_with_ipv4_ipv6_offload_named` is racing against the previous incarnation's still-alive TUN handle, and the kernel rejects the new TUN with the same name silently (warren-exit binary might be calling `set_network_address` on the OLD TUN that no longer has a valid PacketDevice reader behind it).

## What I'd do next (debug session, not in scope for AE.5.Y)

- **Reproduce without restart cycle** : spin a fresh node, deploy binary, run bench — does FIRST-BOOT bench work? If yes, the restart is the trigger.
- **Add `dmesg | grep warren0` check** to the post-restart shell to surface kernel-level TUN errors.
- **Check whether `systemd KillMode=process` or `ExecStop=` cleanup would prevent the stop-sigterm-timeout SIGKILL** and let the binary clean up its Quinn state + TUN device gracefully.
- **Add `RUST_LOG=trace warren-exit` and check whether the accept loop is even looping**.
- **Compare with production warren-exit-1 (130669355)** which has been running 9 days without issue — does its `serve_multihop_with_tun_and_daita` accept loop also break across restart?

## Action items

| Action | Effort | Risk |
|---|---|---|
| Debug session targeting the post-restart silent-fail | 1-2 sessions | Low — production isn't affected (warren-exit-1 has been steady-state for 9 days, no restart) |
| Document the finding for future ops awareness | Done (this report) | — |
| Defer throughput characterization until the silent-fail is understood | Operational — bench numbers are vapor until basic accept loop is reliable | — |
| Continue with non-bench items in the autonomous chain (UI rebrand, hardening lint, etc.) | Per user signal | — |

## Pin

Unchanged from AE.5.X : `5f0bdec`. The AE.5.Y session produced no warren-core code change (only a tear-down + this report).

## Cost

| Item | Cost |
|---|---|
| 3 × ccx13 × ~45 min | ~0.025 EUR |
| Production warren-exit-1 + warren-backend-api | 0 EUR (untouched) |
| Total | ~0.025 EUR (well under cap) |

---

## Open questions for poka

The post-restart silent-fail is the kind of bug that's hard to trigger and easy to misdiagnose. Two paths :

1. **Accept the warren-exit-1 production strategy of "never restart"** : the box has been up for 9 days, the binary works on first-boot. For an AF deploy, freshly provision a new node + cutover DNS / clients, never restart in place.
2. **Invest in fixing the restart hang** : adds operational sanity (rolling updates, hot config reload) but requires diagnostic work first.

Which one fits your operational model? Ping `warren-exit-1` directly and check whether IT has the same issue would resolve this in 30 seconds (you can `sudo systemctl restart warren-exit` on it).
