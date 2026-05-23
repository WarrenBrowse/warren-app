# Session AA — Multi-hop IP negotiation v1 (design)

> Status : **DESIGN — implementation deferred to Sessions AB+**
> Date : 2026-05-22
> Cost réel : **0 EUR**

---

## Problem statement

`warren-client::run_multi_hop_with_tun` currently hardcodes the TUN IPv4 as `10.66.0.2/24` (mono-client POC, Session K.2). The exit-side gateway is hardcoded to `10.66.0.1`. A second client launched against the same exit would either :

- collide with the first client's IP on the exit-side TUN (silent packet routing bug), or
- be rejected by the exit if it does kernel-level conflict detection.

This blocks multi-user production : one warren-exit can today only safely serve ONE simultaneous multi-hop client.

## Goals (v1)

1. Each accepted multi-hop connection gets a **unique IPv4** allocated from a configurable pool on the exit side.
2. The assignment is **communicated to the client in-band** over the existing HPKE-sealed datagram channel (no new transport, no new key, no new endpoint).
3. The client **configures its TUN with the received IP** before the pump loop starts.
4. Allocation **releases on connection close** so the pool doesn't leak under churn.

Out of scope for v1 :
- IPv6 negotiation (TUN already supports dual-stack; defer until v4 path is proven).
- IP renewal / rebind across reconnects (each fresh QUIC connection = fresh allocation; supervisor's reconnect will request a new IP).
- Per-account IP affinity (no stickiness).
- Configurable subnets per exit (single `/24` default, configurable via CLI).

---

## Wire format extension

### Reserved first-byte prefix `0xC0` = "control message"

Current plaintext first-byte semantics inside the HPKE envelope :

| First byte | Meaning |
|---|---|
| `0x40 .. 0x4F` | IPv4 packet (nibble `4`) |
| `0x60 .. 0x6F` | IPv6 packet (nibble `6`) |
| `0xFF` | DAITA dummy (`DAITA_DUMMY_FIRST_BYTE`, Session I) |

Reserved by AA :

| First byte | Meaning |
|---|---|
| `0xC0` | **Warren control message** (postcard-encoded enum follows) |

Rationale : `0xC0` is outside any valid IP version nibble and is not the DAITA dummy marker. Cheap dispatch at the receiver : a single byte compare branches between "real IP packet" / "DAITA dummy" / "control message".

### Control message frame layout

```
+--------+--------+--------+----...---+
| 0xC0   | 0x01   | payload (postcard-encoded WarrenControlMessage)
+--------+--------+--------+----...---+
  marker  version
```

- `0xC0` — fixed prefix; receivers MUST drop the frame as malformed if the version byte is missing.
- `0x01` — control protocol version. Mismatched versions = drop with `tracing::warn!`. Bumped only on breaking wire changes; backward-compat additions go inside the postcard struct via Serde's optional fields.
- Postcard payload — `WarrenControlMessage` enum.

### `WarrenControlMessage` enum (v1)

Lives in a new module `crates/warren-multihop/src/control.rs`.

```rust
/// Control messages exchanged over the HPKE-sealed multi-hop channel,
/// carried as a `WarrenMultihopFrame` whose plaintext starts with the
/// reserved `0xC0` marker (see § wire format).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WarrenControlMessage {
    /// Client -> exit. Optionally requests a specific IPv4 (advisory only;
    /// the exit MAY override). `None` = "give me anything from your pool".
    IpRequest {
        prefer_ipv4: Option<[u8; 4]>,
    },
    /// Exit -> client. Authoritative response. Sent immediately on the
    /// reverse channel after the exit observed the first frame from this
    /// session. Allocator picks from a configurable subnet.
    IpAssign {
        ipv4: [u8; 4],
        prefix_len: u8,
        gateway_ipv4: [u8; 4],
    },
    /// Exit -> client. The exit's pool is exhausted. Client should
    /// terminate the session and surface the error.
    IpExhausted,
}
```

`Serialize` + `Deserialize` use serde + postcard for forward-compat (new variants appended; old clients drop unknown variants).

### Encode / decode helpers

```rust
pub const CONTROL_FIRST_BYTE: u8 = 0xC0;
pub const CONTROL_VERSION_V1: u8 = 0x01;

pub fn encode_control(msg: &WarrenControlMessage) -> Result<Vec<u8>>;
pub fn try_decode_control(plaintext: &[u8]) -> Result<Option<WarrenControlMessage>>;
// Returns Ok(Some) if it parsed; Ok(None) if not a control message
// (first byte != 0xC0 -> caller treats as IP packet); Err on
// malformed control prefix (version mismatch / postcard error).
```

---

## Exit-side allocator

New struct `IpAllocator` in `warren-exit/src/ip_pool.rs` :

```rust
pub struct IpAllocator {
    subnet: Ipv4Net,            // e.g. 10.66.0.0/24
    gateway: Ipv4Addr,          // first host = 10.66.0.1, never allocated
    free: VecDeque<Ipv4Addr>,   // initialised from subnet hosts minus gateway
    used: HashMap<ConnId, Ipv4Addr>,
}

impl IpAllocator {
    pub fn new(subnet: Ipv4Net, gateway: Ipv4Addr) -> Self;
    pub fn allocate(&mut self, conn: ConnId) -> Option<Ipv4Addr>; // None = exhausted
    pub fn release(&mut self, conn: ConnId);
    pub fn gateway(&self) -> Ipv4Addr;
    pub fn prefix_len(&self) -> u8;
}
```

`ConnId` is just a `Quinn::ConnectionId` or a fresh u64 per-conn handle. Shared as `Arc<Mutex<IpAllocator>>` across all spawned conn tasks.

CLI surface on warren-exit :
- `--multihop-subnet <ip/prefix>` (default `10.66.0.0/24`)
- `--multihop-gateway <ip>` (default = first host of subnet)

Exit-side TUN already lives on the gateway IP; the allocator's job is just to give out `.2 .. .254` (or until exhaustion).

---

## Client-side startup reorder

Current `run_multi_hop_with_tun` :

1. Create TUN with `10.66.0.2/24` hardcoded.
2. Spawn supervisor + uplink + downlink pumps.
3. Wait for shutdown.

New ordered startup :

1. Establish QUIC connection (supervisor v1 already does this).
2. **Build a transient client session** that can send/recv ONE control message round-trip.
3. Send `IpRequest { prefer_ipv4: None }` (HPKE-sealed via the established multi-hop session).
4. Receive `IpAssign { ipv4, prefix_len, gateway_ipv4 }` (HPKE-sealed reverse frame, plaintext starts with `0xC0`).
5. Create the `RealTun` with the assigned IPv4.
6. Spawn supervisor's uplink/downlink pumps (existing path, but TUN now uses the negotiated IP).
7. Wait for shutdown.

Bounded by a `tokio::time::timeout(Duration::from_secs(5), ...)` on the IpAssign wait — if exit doesn't answer in 5 s, error out and let supervisor reconnect.

### Pump-side handling of control messages

After AA, both the client's `run_downlink_with_daita` and the exit's `serve_one_connection_with_tun_and_daita` rx loops must dispatch on the plaintext first byte :

```rust
match plaintext.first() {
    Some(&CONTROL_FIRST_BYTE) => /* parse + handle control msg */,
    Some(&DAITA_DUMMY_FIRST_BYTE) => /* DAITA dummy, drop */,
    Some(b) if (b & 0xF0) == 0x40 || (b & 0xF0) == 0x60 => /* IP packet, forward to TUN */,
    _ => /* malformed, drop with warn */,
}
```

In v1, control messages are only sent *during startup* — after the initial IpAssign, neither side emits a `0xC0` plaintext. But the rx dispatch is permanent so future control extensions (per-session DAITA spec, MTU renegotiation, etc.) can land without touching the dispatch logic.

---

## Test surface (per session)

### Session AB — Wire format + encode/decode

- `crates/warren-multihop/tests/control_message_v1.rs` :
  - Round-trip each `WarrenControlMessage` variant through `encode_control` / `try_decode_control`.
  - Reject `0xC0` followed by `0x02` (unknown version).
  - Pass-through : non-`0xC0` prefix returns `Ok(None)`.
  - Reject truncated postcard payload.

### Session AC — Allocator

- `crates/warren-exit/tests/ip_pool_basic.rs` :
  - Allocate up to subnet capacity then expect `None` on overflow.
  - Release returns the IP to the free queue.
  - Gateway IP never allocated.

### Session AD — End-to-end pump integration

- `crates/warren-exit/tests/multihop_tun_with_ip_nego.rs` :
  - Spawn 2 simulated clients against one exit, assert they get distinct IPs.
  - Assert the first frame from each client is an `IpRequest`, the first reverse frame is an `IpAssign`.

### Session AE — Production wire-up

- `warren-client::main::run_multi_hop_with_tun` : reorder + TUN-after-IpAssign.
- `warren-exit::main` : allocator surface, CLI flags.
- Hetzner re-bench (1 session, ~0.02 EUR) to confirm overhead is unchanged.

---

## Risks + open questions

1. **TUN creation timing on Linux / macOS** : `RealTun::create_with_ipv4_mtu` issues blocking syscalls (`ioctl SIOCSIFADDR`, route install). Moving TUN creation *after* QUIC handshake adds latency to first packet. Mitigation : create TUN with a placeholder IP `0.0.0.0/0` immediately, then reconfigure via `ip addr add` after IpAssign. Need a `RealTun::reassign_ipv4` method.
2. **Supervisor reconnect semantics** : `MultiHopSupervisor` currently swaps `Connection`s under the watch channel without re-negotiating IP. After AA, every fresh `Connection` MUST trigger a new IpRequest/IpAssign round-trip, and the client TUN MUST be reconfigured if the exit allocates a different IP. Alternative : exit-side persistence by client pubkey (stickiness) — out of v1.
3. **HPKE session reset on rekey** : when the client rotates `encapsulated_key`, the exit's `SessionCache` may reuse the IP from the prior session or allocate fresh. v1 keeps it simple — IP follows the QUIC `Connection`, not the HPKE session, and rekey within a single QUIC conn does not trigger IpRequest.
4. **Control plaintext under DAITA** : DAITA dummies are still 0xFF on the wire. Control messages (0xC0) are real packets that count as `NormalSent`/`NormalRecv` in the maybenot framework. Acceptable v1 — DAITA observers see exactly 2 small "real packets" at handshake (the IP exchange), which is indistinguishable from any other tiny IP packet (e.g. a TCP SYN).

---

## Sequencing across future sessions

| Session | Deliverable | Cost cap |
|---|---|---|
| **AA** (this) | Design markdown | 0 EUR |
| **AB** | `WarrenControlMessage` + encode/decode + tests | 0 EUR (in-process) |
| **AC** | `IpAllocator` + tests | 0 EUR |
| **AD** | rx dispatch wiring + 2-client integration test | 0 EUR |
| **AE** | Production wire-up + CLI flags + bench validation | ~0.02 EUR Hetzner |
| **AF** | Production deploy + warren-exit-1 redeploy | 0 EUR (ops) |

---

## Doctrine

- §0.0 INVIOLABLE git respecté — no destructive working-tree commands used.
- §0.5 plein mandat — Session AA scope strictly bounded to design (no production code touched).
- §0.6 worktree — not used for this session (design-only markdown lives in `warren-app/.planning/`).

---

## Pin warren-app

Unchanged from Session Z (`69d8c00`). No warren-core code changes in this session.
