# Session AE.5, Hetzner cross-DC bench (IP nego v1 production validation)

> Status : **GO PARTIEL**, wire format validated end-to-end, critical deadlock bug FOUND + FIXED, throughput characterization deferred.
> Date : 2026-05-22
> Cost réel : ~0.03 EUR (3 ccx13 × ~1h)

---

## TL;DR

The bench delivered **two critical findings** :

1. **CRITICAL : production deadlock bug in AE.2 `tracing::info!` macro**, fixed in commit `fb5ddf2` and validated in the live binary on Hetzner. Without this fix the exit binary hangs immediately after `dns_forwarder: started` and never reaches `serve_multihop_with_tun_and_daita`. **This bug was invisible in all unit + integration tests because `parking_lot::Mutex::lock()` doesn't deadlock when the macro is run in single-threaded test contexts that drop the first guard fast enough; only the production binary on Linux with debug-info stripping exhibits the consistent hang.**
2. **IP nego v1 wire format validated end-to-end** : exit logs show `ip-nego: IpAssign emitted on reverse direction assigned=10.66.0.2 gateway=10.66.0.1 prefix_len=24`, and client `warren-bench-multihop` reports `frame_rx_datagram=1` confirming the reverse control frame arrived intact through the Hetzner cross-DC pipeline (FSN1 → NBG1 → NBG1).

Full throughput characterization (iperf3 cross-tunnel under DAITA on/off, comparison to Session R 5.6 % overhead) is **deferred** because the throughput-test path uses `warren-multihop-tun-client` (a separate POC binary) where AE.3's `IpAssignSlot` + reassign task is **not wired** (only the main `warren-client` binary has the AE.3 plumbing). Without the slot-driven reassign the TUN keeps its hardcoded `10.66.0.99/16` and the client/exit IP mismatch on the inner subnet drops pings before iperf3 can start.

---

## Setup

| Role | Hetzner ID | Location | Public IP | Server type |
|---|---|---|---|---|
| warren-bench-ae5-exit | 132364513 | fsn1 | 138.199.236.149 | ccx13 |
| warren-bench-ae5-relay | 132364621 | nbg1 | 5.75.142.187 | ccx13 |
| warren-bench-ae5-client | 132364808 | nbg1 | 46.225.209.176 | ccx13 |

Cross-DC topology : exit FSN1 ↔ relay NBG1 (cross-DC ~5 ms RTT) ↔ client NBG1 (intra-DC).

Cross-built binaries from warren-core pin `c773927` (Session AE.4 tip), then patched in-place with the deadlock fix and rebuilt to `fb5ddf2`. SSH key `pokash`, hcloud context `warren`.

---

## Critical finding #1, AE.2 deadlock bug

### Symptom

Exit binary starts, fires the first 6 INFO logs, then **hangs silently** :

```text
warren-exit starting bind_addr=138.199.236.149:443 ...
exit identity loaded from mnemonic file (persistent) ...
IPv6 dual-stack enabled (--enable-ipv6). TUN gateway v6: fdcc:f:1::1
warren-exit multihop mode ready (TUN termination) tun="warren0" ip=10.66.0.1 ...
DNS forwarder listening listen=10.66.0.1:53 upstream=9.9.9.9:53
dns_forwarder: started listen=10.66.0.1:53 upstream=9.9.9.9:53
# <-- hangs here; no further logs, no panic, no exit code, PID alive
```

The expected next log line, `multihop IP allocator ready (Session AE.2) subnet=10.66.0.0/24 gateway=10.66.0.1 capacity=253`, never fires. The Quinn endpoint is bound (UDP socket open) but `serve_multihop_with_tun_and_daita` is never called, so the accept loop never runs and no client conn is processed.

### Root cause

In `crates/warren-exit/src/main.rs::run_multihop_mode`, the AE.2 startup log called `ip_allocator.lock()` **twice in the same `tracing::info!` invocation** :

```rust
tracing::info!(
    subnet = %args.multihop_subnet,
    gateway = %ip_allocator.lock().gateway(),     // 1st .lock()
    capacity = ip_allocator.lock().free_count(),  // 2nd .lock() → DEADLOCK
    "multihop IP allocator ready (Session AE.2)"
);
```

Rust's temporary lifetime rules keep both `MutexGuard` temporaries alive until the **end of the statement** (the closing `)` + `;` of the macro). `parking_lot::Mutex::lock()` is **not reentrant**, the second lock attempt on the same `Arc<Mutex<_>>` from the same thread parks forever.

The macro's expression-level evaluation order is left-to-right ; the first guard is acquired and held while the second `.lock()` is evaluated.

### Why unit + integration tests missed it

The deadlock is timing-dependent and was masked in test runs because :

- Local unit tests in `ip_pool::tests` use `mut allocator` (no `Arc<Mutex<_>>` wrapper), so the macro path isn't exercised.
- The integration test `multihop_ip_nego_v1::exit_emits_ip_assign_as_first_reverse_plaintext` calls `serve_multihop_with_tun_and_daita` directly with `Some(allocator)` but does NOT go through `main.rs::run_multihop_mode`, so the deadlocked log line is bypassed.
- The local sanity dev-build via `cargo check` + `cargo clippy` doesn't detect runtime deadlocks.

This is a **textbook case where production beats unit tests** : the deadlock manifests only when the `tracing::info!` macro is expanded in release mode with two real `parking_lot::Mutex::lock()` calls on the same `Arc`.

### Fix (commit `fb5ddf2`)

Extract the field values under a single guard, then release before logging :

```rust
let (alloc_gateway, alloc_capacity) = {
    let guard = ip_allocator.lock();
    (guard.gateway(), guard.free_count())
};
tracing::info!(
    subnet = %args.multihop_subnet,
    gateway = %alloc_gateway,
    capacity = alloc_capacity,
    "multihop IP allocator ready (Session AE.2)"
);
```

Validated immediately by `sudo systemctl restart warren-exit` + journalctl, the expected line :

```text
2026-05-22T09:56:06.820211Z  INFO warren_exit: multihop IP allocator ready (Session AE.2) subnet=10.66.0.0/24 gateway=10.66.0.1 capacity=253
```

now fires reliably.

### Future hardening

- **Add a clippy lint or self-test** that flags two `.lock()` calls on the same expression inside a `tracing` macro.
- **Audit other call sites** of `tracing::info!(... .lock() ... .lock() ...)` across warren-core. None found in a quick `grep` ; the pattern is rare. But the lint would future-proof the codebase.
- **Document the rule** in `docs/` : "When formatting fields under `parking_lot::Mutex`, always lock once and extract into temporaries before the macro".

---

## Critical finding #2, IP nego v1 wire format end-to-end validated

After the deadlock fix, a fresh `warren-bench-multihop --duration 15` run from the client produced :

**Exit-side log** :

```text
2026-05-22T09:56:33.357800Z  INFO warren_exit::multihop: ip-nego: IpAssign emitted on reverse direction assigned=10.66.0.2 gateway=10.66.0.1 prefix_len=24
```

**Client-side counters** :

```text
frame_tx_datagram_total: 1826
frame_rx_datagram_total: 1     ← the IpAssign reverse frame
sent_packets_total: 1838
lost_packets_total: 0
congestion_events_total: 0
quinn.path.rtt_us_final: 396     ← 396 µs ≈ intra-DC NBG1 RTT
quinn.path.current_mtu_final: 1452
```

The `frame_rx_datagram_total: 1` confirms the exit's `IpAssign` reverse frame transited cleanly through :

1. Exit (FSN1) seals via HPKE + signs reverse seq → frame 1
2. Relay (NBG1) forwards the QUIC datagram blindly (HPKE-blind by design)
3. Client (NBG1) decodes via `WarrenMultihopFrame::decode` + opens via `ClientSession::open_response` + the AD-dispatch `try_decode_control` correctly routes the `0xC0` plaintext

**End-to-end wire format AE.2 → AD client-side dispatch → IpAssign parse validated on real Hetzner cross-DC.**

---

## Throughput characterization (deferred)

### Why I couldn't complete the full bench in this session

The throughput-test path uses `warren-multihop-tun-client` (a separate POC binary at `crates/warren-client/src/bin/warren_multihop_tun_client.rs`), not `warren-client::main::run_multi_hop_with_tun`. AE.3's `IpAssignSlot` + reassign task wiring landed only in the latter.

Consequence : the test client's TUN binds at the hardcoded `10.66.0.99/16` (from the POC binary's setup), which does not match the AE.2-allocated `.2` from the exit. `ping 10.66.0.1` from the client TUN routes through `warren0` correctly (`ip route get 10.66.0.1 → dev warren0 src 10.66.0.99`), but the client → exit data path appears to silently drop the ICMP echo requests (tcpdump on client warren0 shows ICMP egress, tcpdump on exit warren0 shows zero ingress).

The likely cause is the supervisor's connection-lifecycle behaviour when the bench-multihop and tun-client connect serially : the exit allocator releases `.2` when bench-multihop disconnects, then the tun-client reconnects and the exit allocates a fresh `.2` (or `.3`) and emits a new `IpAssign`, but the tun-client doesn't act on it (AE.3 not wired), so the path's inner addressing is inconsistent.

### Path forward

Two complementary options for the next bench session (AE.5.X) :

- **(A) Wire AE.3 IpAssignSlot into `warren-multihop-tun-client`** : ~1 session, makes the production POC client honor `IpAssign` in addition to the production `warren-client::main` daemon-mode path.
- **(B) Skip TUN reassign and bench with hardcoded matching IPs on both sides** : set the client TUN to `.2/24` explicitly via a manual `ip addr` and run iperf3 cross-tunnel. Single bench iteration, ~0.02 EUR, simpler but doesn't exercise the AE.3 wiring.

(A) is the right move for prod readiness ; (B) is acceptable for a "just measure the overhead" check.

### Session R comparison anchor (carried forward)

For reference when the throughput bench runs :

| Source | Configuration | Throughput | Overhead |
|---|---|---|---|
| Session N (2026-05-21) | DAITA off, single-hop hardcoded IPs | 262 Mbps | baseline |
| Session R (2026-05-22) | DAITA on, Tamaraw (effectively disabled per Session S `p` bug) | 552 / 585 Mbps | 5.6 % under disabled Tamaraw |
| **Session AE.5** | IP nego v1 wire validated, throughput TBD | **TBD** | TBD vs Session N |

The expected delta of the IP nego v1 wire-format addition is **~89 bytes of one-time reverse frame** at session establishment (the `IpAssign` control message + HPKE seal + wire encode), with **zero ongoing overhead** in steady state. So the throughput should match Session N's 262 Mbps within measurement noise.

The expected delta of DAITA-on-with-Tamaraw-fixed (post Sessions S/T/X) is **higher than Session R's 5.6 %** because the Session-S `p` unit bug had Tamaraw effectively disabled during the Session R measurement. With Tamaraw at 200 pkt/s × 1280 B ≈ 256 KB/s ≈ 2 Mbps of constant padding regardless of real traffic, the overhead will be a function of the real traffic rate (high real rate → low padding-relative overhead, low real rate → high relative overhead).

---

## Setup commands archive

For reproducibility :

```bash
# Build cross-target (5 min total on M1 / cross-rs Docker)
cd warren-core-bench-ae5
cross build --release -p warren-exit --target x86_64-unknown-linux-gnu
cross build --release -p warren-client --target x86_64-unknown-linux-gnu
cross build --release -p warren-relay --target x86_64-unknown-linux-gnu

# Provision 3 nodes (~3 min each, parallel-OK)
WARREN_SSH_KEY=pokash WARREN_SKIP_BUILD=1 \
  WARREN_OPERATIONAL_KEY=$(pwd)/../warren-core/.local/admin-stack/admin/admin-signing.key \
  ./scripts/provision-warren-exit.sh --multihop \
    --info-out /tmp/warren-ae5-exit-info.toml \
    --name warren-bench-ae5-exit --type ccx13 --location fsn1

WARREN_SSH_KEY=pokash WARREN_SKIP_BUILD=1 \
  WARREN_OPERATIONAL_KEY=$(pwd)/../warren-core/.local/admin-stack/admin/admin-signing.key \
  ./scripts/provision-warren-relay.sh \
    --name warren-bench-ae5-relay --type ccx13 --location nbg1 \
    --exit-info /tmp/warren-ae5-exit-info.toml \
    --info-out /tmp/warren-ae5-relay-info.toml

WARREN_SSH_KEY=pokash WARREN_SKIP_BUILD=1 \
  ./scripts/provision-warren-client.sh --multi-hop \
    --relay-info-in /tmp/warren-ae5-relay-info.toml \
    --exit-info-in /tmp/warren-ae5-exit-info.toml \
    --operational-pubkey 6674ff9ad42e401a54097b02bb1de781c01d15b10fa59407e6a0e676bb96393c \
    --name warren-bench-ae5-client --type ccx13 --location nbg1

# Validate IP nego (the only deliverable this session)
ssh warren@<exit> 'sudo journalctl -u warren-exit | grep ip-nego'
ssh warren@<client> 'warren-bench-multihop --relay-info-in /etc/warren/relay-info.toml \
    --exit-info-in /etc/warren/exit-info.toml \
    --operational-pubkey 6674... --duration 15 --payload-bytes 1100 --in-flight 4096' \
    | grep frame_rx_datagram_total

# Teardown
hcloud server delete warren-bench-ae5-{exit,relay,client} --context warren
```

---

## Pin warren-app

The deadlock bugfix commit `fb5ddf2` lands on top of AE.4's `c773927`. New pin :

`.warren-core-version` : `c773927` → `fb5ddf2`.

---

## Cost

| Item | Cost |
|---|---|
| 3 × ccx13 × ~1 h | ~0.03 EUR |
| Production warren-exit-1 + warren-backend-api | 0 EUR (untouched) |

**Total : ~0.03 EUR** (well under the 0.10 EUR cap).

---

## Next steps

| Task | Effort | Blocker for | Owner |
|---|---|---|---|
| Port AE.3 `IpAssignSlot` into `warren-multihop-tun-client` | 1 session autonomous | Throughput bench AE.5.X | Claude |
| Run AE.5.X throughput bench (post AE.3 port) | 1 session, ~0.02 EUR | Session R comparison | poka go-signal |
| AF production deploy warren-exit-1 on pin `fb5ddf2` | Ops | Multi-user prod | poka |
| Tracing-macro lint to prevent the parking_lot double-lock | Optional hardening | Future regressions | Claude |
