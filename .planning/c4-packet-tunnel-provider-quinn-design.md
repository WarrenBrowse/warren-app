# C.4 PacketTunnelProvider Quinn design — iOS NetworkExtension over warren-tunnel

Design document for sub-phase C.4 of Session C iOS fork brief. Replaces the
upstream Mullvad WireGuardAdapter pattern with a Warren Quinn-based packet
tunnel provider backed by the warren-tunnel crate (warren-core).

**Status** : C.4.0 DELIVERED (warren-core IosTun + warren-ios FFI handle
lifecycle + Swift WarrenQuinnAdapter wire-in), C.4.1+ TODO.
Effort estimate brief : 10-14 days wall-clock. Probably underestimated due to
NetworkExtension iOS subtleties (Wi-Fi <-> cellular handover, App Group event
broadcasting, killswitch via Disconnect on Demand).

### C.4.0 delivered (2026-05-21)

- `warren-core::IosTun` (`warren-core/crates/warren-tunnel/src/ios_tun.rs`,
  157 LOC) — `PacketDevice` impl bridging `NEPacketTunnelFlow` via
  symmetric inbound/outbound mpsc channels (no raw fd, no unsafe). 5
  host unit tests cover round-trip + clone-shared-channels + try_recv
  empty.
- `warren-ios::warren_tunnel_ffi` refactor — Box<Arc<WarrenTunnelHandleImpl>>
  lifecycle, multi-thread Tokio runtime, IosTun ownership, spawned
  outbound dispatcher draining `IosTun::next_outbound`, atomic state
  counters surfaced via `warren_tunnel_status`.
- 3 new FFI entry points :
  - `warren_tunnel_inject_inbound_packet(handle, data, len)` — Swift →
    Rust uplink push (consumed by `readPackets` loop).
  - `warren_tunnel_set_outbound_callback(handle, cb, ctx)` — registers
    plain-C-fn-pointer downlink callback (Rust dispatcher → Swift
    `writePackets`).
  - `warren_tunnel_set_event_callback(handle, cb, ctx)` — registers
    tagged-event callback (Connected / Disconnected / Reconnecting /
    Failover / NatPmp*).
- Swift `WarrenQuinnAdapter` (`ios/WarrenRustRuntime/WarrenQuinnAdapter.swift`,
  390 LOC) — final class (not actor for callback ergonomics),
  `Unmanaged.passRetained` self-ref as FFI context, `@convention(c)`
  `outboundCallback` and `eventCallbackBridge` mapping C → Swift enum
  variants, inbound `Task` looping on `packetFlow.readPackets`, IPv4/IPv6
  protocol auto-detect via first-nibble header inspection, single-hop
  marshalling (multi-hop + DAITA C-struct pinning deferred C.4.1).
- Pin warren-core `732869d` → `8779843`.

### C.4.1+ remaining work

The C.4.0 plumbing is in place ; what's still needed :

1. **Quinn handshake** : `warren_tunnel_start` body currently allocates
   handle + spawns outbound dispatcher but does not initiate a Quinn
   connection. Wire `warren_tunnel::ClientTunnel::connect` with the
   marshalled parameters. Needs `Connecting` → `Connected` state
   transition + event dispatch.
2. **Multi-hop + DAITA marshalling** : extend `WarrenQuinnAdapter.callTunnelStart`
   to pin `WarrenRelayConfigC` + `WarrenDaitaSpecC` C structs across
   the FFI call boundary (current single-hop pass uses `nil`).
3. **Event dispatch from Rust side** : the `handle_impl.event_callback`
   Mutex storage is wired ; the future Quinn connection task must
   actually invoke the stored callback when state transitions happen.
4. **PacketTunnelProvider rewrite** : add `WarrenQuinnTunnelImplementation`
   conforming to `TunnelImplementation` protocol so it slots into the
   existing dispatcher next to `WireGuardGoTunnelImplementation` /
   `GotaTunTunnelImplementation`. Toggle via `PacketTunnelDebugSettings.useWarrenQuinn`
   debug flag first, then remove the WG path for the production build.
5. **WireGuardKit removal** : drop `WireGuardKitTypes` + `WireGuardKit`
   framework refs from pbxproj after all WG consumers (WgAdapter,
   TunnelPinger, BlockedStateErrorMapper unused import) are stubbed
   or removed. Resolves the standing "Unexpected duplicate tasks"
   linker error in `xcodebuild build -target WarrenVPN`.

**Pre-conditions** :
- C.1 + C.2 + C.3 skeleton DONE (cf. `.planning/session-c-report.md`).
- warren-ios crate exists with FFI module skeletons (`warren_tunnel_ffi`,
  `warren_wallet_ffi`, `warren_multihop_ffi`, `warren_natpmp_ffi`).
- C.3 deep work (api_client mullvad → warren rewrite + FFI implementations)
  recommended completed first, OR done in parallel here.

---

## 1. Current Mullvad architecture (what we replace)

### 1.1 Class hierarchy

```
NEPacketTunnelProvider (NetworkExtension)
  └─ PacketTunnelProvider (iOS/PacketTunnel/PacketTunnelProvider/PacketTunnelProvider.swift, 369 lines)
       └─ implementation: TunnelImplementation (protocol abstraction)
            ├─ WireGuardGoTunnelImplementation (iOS/PacketTunnel/PacketTunnelProvider/WireGuardGoTunnelImplementation.swift, 223 lines)
            │    └─ WgAdapter (iOS/PacketTunnel/WireGuardAdapter/WgAdapter.swift, 260 lines)
            │         └─ WireGuardAdapter (from WireGuardKit external Swift package)
            └─ GotaTunTunnelImplementation (PacketTunnelCore alternative, debug)
```

### 1.2 Key responsibilities of PacketTunnelProvider

- **Lifecycle** : `startTunnel(options:completionHandler:)` ←→
  `stopTunnel(with:completionHandler:)` (NetworkExtension lifecycle)
- **Settings reading** : reads from App Group container URL (via
  `SettingsReader`) to get exit relay, account number, multi-hop config, …
- **Migration** : runs `MigrationManager` on every start (settings schema
  versioning)
- **API context** : maintains a `MullvadApiContext` for in-tunnel API calls
  (device check, access methods, …)
- **Access method receiver** : `MullvadAccessMethodReceiver` swaps API access
  methods based on tunnel state
- **Device checker** : `DeviceChecker` enforces device limit (max 5 per
  account)
- **Shadowsocks cache cleaner** : invalidates Shadowsocks bridge cache
- **Logging** : flushes logs to App Group container

### 1.3 Key responsibilities of WireGuardGoTunnelImplementation

Conforms to `TunnelImplementation`. Calls into `WgAdapter` which wraps
WireGuardKit's `WireGuardAdapter`. Manages :
- WireGuard handshake + key exchange (PostQuantum optional via
  `EphemeralPeerExchangingPipeline` from PacketTunnelCore)
- Connection-mode rotation (UDP <-> TCP-over-Shadowsocks <-> ChannelLink)
- Reconnect on network path change (cellular <-> Wi-Fi)
- Stats reporting (RX/TX bytes)

### 1.4 What is shared with Warren and what must change

| Item | Reuse | Replace | Notes |
|------|-------|---------|-------|
| `NEPacketTunnelProvider` parent class | ✅ | - | iOS framework, mandatory |
| `PacketTunnelProvider` subclass | partial | partial | Keep settings-reader + App Group + logging plumbing ; replace `implementation` instantiation |
| `TunnelImplementation` protocol | ✅ | - | Generic abstraction, useful for swap |
| `WgAdapter` | - | ✅ DROP | WireGuardKit-specific |
| `WireGuardGoTunnelImplementation` | - | ✅ DROP (and `GotaTunTunnelImplementation`) | WG-specific |
| `EphemeralPeerExchangingPipeline` + PostQuantum | - | ✅ DROP | Warren uses HPKE via warren-multihop (cf. M5.B.1) |
| `MullvadApiContext` | - | ⚠️ REPLACE | Warren uses `WarrenApiClient` from warren-core (canonical-message signed requests) |
| `MullvadAccessMethodReceiver` | - | ❓ DROP | Mullvad-specific (bridge / encrypted DNS rotation). Warren has M4.0 baseline obfuscation in warren-tunnel, no need for in-tunnel access method swap |
| `DeviceChecker` | - | ❓ DROP / REPLACE | Mullvad enforces device count via account-number API. Warren auth = wallet, device concept TBD |
| `ShadowsocksCacheCleaner` | - | ✅ DROP | Shadowsocks unused in Warren |
| `MigrationManager` | partial | partial | Keep settings migration infra, but settings schema changes (account_number → wallet) |
| Logging (`MullvadLoggingSubscriber`) | ✅ | rename | Already `WarrenLogging` post-C.2 |
| `SettingsReader` | partial | partial | Keep but reads `WarrenTunnelSettings` (new schema with wallet + multi-hop + DAITA + NAT-PMP) |
| `NEPacketTunnelFlow` plumbing | ✅ | - | iOS framework reader/writer for IP packets, mandatory |

---

## 2. Target Warren architecture

### 2.1 New class hierarchy

```
NEPacketTunnelProvider (NetworkExtension)
  └─ PacketTunnelProvider (iOS/PacketTunnel/PacketTunnelProvider/PacketTunnelProvider.swift, rewritten)
       ├─ implementation: TunnelImplementation
       │    └─ WarrenQuinnTunnelImplementation (NEW)
       │         └─ WarrenQuinnAdapter (NEW)
       │              └─ warren_tunnel_ffi (Rust FFI from warren-ios crate)
       │                   └─ warren-tunnel crate (Quinn connection, HPKE multi-hop, DAITA, NAT-PMP)
       ├─ apiClient: WarrenApiClient (NEW, replaces MullvadApiContext)
       │    └─ warren_api_client_ffi (Rust FFI from warren-ios crate)
       │         └─ warren-api-client crate
       ├─ wallet: WarrenWallet (NEW)
       │    └─ warren_wallet_ffi (Rust FFI from warren-ios crate)
       │         └─ warren-identity crate
       └─ portForwarding: WarrenPortForwarding (NEW, optional)
            └─ warren_natpmp_ffi (Rust FFI from warren-ios crate)
                 └─ warren-natpmp-client crate
```

### 2.2 WarrenQuinnAdapter Swift contract

```swift
import Foundation
import NetworkExtension
import WarrenRustRuntime  // C FFI from warren-ios

public actor WarrenQuinnAdapter {
    private var handle: OpaquePointer?  // WarrenTunnelHandle from FFI
    private let packetFlow: NEPacketTunnelFlow
    private let eventCallback: (WarrenTunnelEvent) -> Void

    public init(
        packetFlow: NEPacketTunnelFlow,
        eventCallback: @escaping (WarrenTunnelEvent) -> Void
    ) {
        self.packetFlow = packetFlow
        self.eventCallback = eventCallback
    }

    public func start(config: WarrenTunnelConfig) async throws {
        // Marshal Swift config -> WarrenTunnelParameters C struct
        // Call warren_tunnel_start(parameters, packet_fd) via FFI
        // Spawn a Task that reads packets from packetFlow.readPackets,
        // writes via FFI ; and from FFI events writes back via
        // packetFlow.writePackets.
    }

    public func stop() async {
        // Call warren_tunnel_stop(handle) via FFI
    }

    public func reconnect() async {
        // Call warren_tunnel_reconnect(handle) via FFI
        // (used on NEPacketTunnelProvider.handleEvents .pathChanged)
    }

    public func status() async -> WarrenTunnelStatus {
        // Call warren_tunnel_status(handle, &status) via FFI
    }
}

public struct WarrenTunnelConfig {
    public let exitPubkey: Data  // 32 bytes Ed25519
    public let exitEndpoint: String  // "IP:port"
    public let walletSigningKey: Data  // 32 bytes Ed25519 derived from BIP39
    public let multiHopRelay: WarrenRelayConfig?  // optional entry relay
    public let daitaSpec: WarrenDaitaSpec?  // optional DAITA
    public let natPmpEnabled: Bool
    public let bypassCidrs: [String]  // optional --bypass-cidr
}

public enum WarrenTunnelEvent {
    case connected
    case disconnected
    case reconnecting
    case failover(toExit: String)
    case natPmpMapped(internalPort: UInt16, externalPort: UInt16, lifetime: UInt32)
    case natPmpRenewed(externalPort: UInt16)
    case natPmpFailed(reason: String)
}
```

### 2.3 PacketTunnelProvider rewrite outline

```swift
import Foundation
import WarrenLogging
import WarrenREST  // legacy until C.3 deep step 2 replaces with WarrenApiClient
import WarrenRustRuntime
import WarrenSettings
import WarrenTypes
@preconcurrency import NetworkExtension
import PacketTunnelCore

class PacketTunnelProvider: NEPacketTunnelProvider, @unchecked Sendable {
    private let internalQueue = DispatchQueue(label: "WarrenPacketTunnel.internalQueue")
    private let providerLogger: Logger
    private var adapter: WarrenQuinnAdapter!
    private var pathObserver: PacketTunnelPathObserver!

    override init() {
        Self.configureLogging()
        providerLogger = Logger(label: "WarrenPacketTunnelProvider")
        super.init()
    }

    override func startTunnel(
        options: [String: NSObject]? = nil,
        completionHandler: @escaping (Error?) -> Void
    ) {
        Task {
            do {
                let settings = try TunnelSettingsManager(...).read().tunnelSettings
                let config = try WarrenTunnelConfig(from: settings)
                adapter = WarrenQuinnAdapter(
                    packetFlow: packetFlow,
                    eventCallback: { [weak self] event in
                        self?.broadcastEvent(event)
                    }
                )
                try await adapter.start(config: config)
                pathObserver = PacketTunnelPathObserver { [weak self] _ in
                    Task { await self?.adapter.reconnect() }
                }
                completionHandler(nil)
            } catch {
                providerLogger.error("Failed to start: \(error)")
                completionHandler(error)
            }
        }
    }

    override func stopTunnel(
        with reason: NEProviderStopReason,
        completionHandler: @escaping () -> Void
    ) {
        Task {
            await adapter?.stop()
            pathObserver?.stop()
            completionHandler()
        }
    }

    private func broadcastEvent(_ event: WarrenTunnelEvent) {
        // Write to App Group shared UserDefaults so main app gets notified.
        // Pattern documented in memory `warren_session_b_delivered` (M5.B.2
        // failover banner uses similar mechanism).
        let defaults = UserDefaults(suiteName: ApplicationConfiguration.securityGroupIdentifier)
        switch event {
        case .connected:
            defaults?.set(Date(), forKey: "WarrenTunnel.lastConnectedAt")
        case .failover(let toExit):
            defaults?.set(toExit, forKey: "WarrenTunnel.lastFailoverExit")
            defaults?.set(Date(), forKey: "WarrenTunnel.lastFailoverAt")
            // Increment failover_count for UI banner (cf. M5.B.2)
            let prev = defaults?.integer(forKey: "WarrenTunnel.failoverCount") ?? 0
            defaults?.set(prev + 1, forKey: "WarrenTunnel.failoverCount")
        // ... etc.
        }
    }
}
```

---

## 3. warren_tunnel_ffi.rs implementation contract (Rust side)

Documented in `warren-ios/src/warren_tunnel_ffi.rs` skeleton; concrete
exports for C.4 :

```rust
#[unsafe(no_mangle)]
pub extern "C" fn warren_tunnel_start(
    parameters: *const WarrenTunnelParametersC,
    packet_fd: i32,
) -> *mut WarrenTunnelHandle { ... }

#[unsafe(no_mangle)]
pub extern "C" fn warren_tunnel_stop(handle: *mut WarrenTunnelHandle) { ... }

#[unsafe(no_mangle)]
pub extern "C" fn warren_tunnel_reconnect(handle: *mut WarrenTunnelHandle) { ... }

#[unsafe(no_mangle)]
pub extern "C" fn warren_tunnel_status(
    handle: *mut WarrenTunnelHandle,
    out_status: *mut WarrenTunnelStatusC,
) -> i32 { ... }

#[unsafe(no_mangle)]
pub extern "C" fn warren_tunnel_set_event_callback(
    handle: *mut WarrenTunnelHandle,
    callback: unsafe extern "C" fn(*const WarrenTunnelEventC, *mut c_void),
    context: *mut c_void,
) { ... }

#[repr(C)]
pub struct WarrenTunnelParametersC {
    pub exit_pubkey: [u8; 32],
    pub exit_endpoint: *const c_char,  // "IP:port" UTF-8 NUL-terminated
    pub wallet_signing_key: [u8; 32],
    pub multi_hop_relay: *const WarrenRelayConfigC,  // null if single-hop
    pub daita_spec: *const WarrenDaitaSpecC,  // null if DAITA OFF
    pub nat_pmp_enabled: bool,
    pub bypass_cidrs: *const *const c_char,
    pub bypass_cidrs_count: u32,
}
```

### 3.1 Integration with warren-tunnel crate

The Rust side spawns a Tokio task that :
1. Builds a `WarrenTunnelParameters` from the C struct (cf. `warren-tunnel::WarrenTunnelParameters`).
2. Constructs a `quinn::Endpoint` bound to the device's outbound interface.
3. Establishes the Quinn connection (single-hop) OR initiates the HPKE
   multi-hop handshake (`warren-multihop::MultiHopClient`) if `multi_hop_relay`
   is set.
4. Spawns a `pump_bidirectional` (single-hop) or
   `pump_multi_bidirectional_with_daita` (multi-hop + DAITA, cf. M5.B.1)
   task that bridges IP packets between `packet_fd` (NEPacketTunnelFlow utun
   fd) and the Quinn connection.
5. Optionally requests a NAT-PMP mapping via
   `warren-natpmp-client::request_mapping`.
6. Optionally subscribes to failover events
   (`warren-relay-selector::report_exit_down`, cf. M5.B.2).
7. Forwards all events to the Swift side via the registered callback.

### 3.2 iOS-specific challenges

- **utun fd handling** : iOS NetworkExtension exposes `NEPacketTunnelFlow`,
  not a raw fd. The fd must be extracted via private API or via
  `NEPacketTunnelFlow.value(forKey: "socket.fileDescriptor")` (works on iOS
  16+ but private and may break on future iOS). Alternative : use
  NEPacketTunnelFlow's `readPackets` / `writePackets` async API and bridge
  to a Rust-side channel that warren-tunnel can read/write through.

  Recommended : use the `readPackets` / `writePackets` bridge via a small
  Rust-managed memory queue, sidestepping fd extraction. Tradeoff : extra
  copy per packet (~1-2 µs at line rate), but no private API.

- **`PacketDevice::from_fd(OwnedFd)` blocker** (cf. Session D Android memory
  `warren_session_d_delivered`) : `tun_rs 2.8` lacks an iOS backend
  (similar to Android blocker). Without a `tun_rs` iOS backend, warren-tunnel
  cannot directly own the utun fd. The `readPackets` / `writePackets` bridge
  approach above sidesteps this.

  Alternatively, contribute an iOS backend to `tun_rs` upstream. Effort
  ~3-5 days, low value if the bridge approach works.

- **Wi-Fi <-> cellular handover** : `PacketTunnelPathObserver`
  (kept from Mullvad) observes `Network.framework.NWPathMonitor` events.
  On path change, call `adapter.reconnect()` which triggers
  `warren_tunnel_reconnect` which calls
  `warren-backoff::Backoff::HANDSHAKE` (15s reconnect window, cf. M4.H.G)
  to re-establish the Quinn connection over the new path.

- **Killswitch / Disconnect on Demand** : NetworkExtension automatically
  blocks traffic when the tunnel is down (default behavior). No additional
  Swift code needed. Verify that iOS's "Disconnect on Demand" rules in the
  `NETunnelProviderManager` config don't accidentally tear down the tunnel
  on multi-hop relay change.

- **Background runtime** : iOS aggressively suspends the extension when
  no traffic flows. NetworkExtension keeps it alive while `startTunnel`
  was called and not yet `stopTunnel`'d, but Tokio tasks may experience
  long pauses on backgrounding. Heartbeat / keep-alive packets every 25s
  recommended (warren-tunnel already does this via Quinn's
  `keep_alive_interval`).

---

## 4. Migration steps (per-commit plan)

### C.4.1 Scaffold WarrenQuinnAdapter Swift class (1-2 days)
- Create `ios/PacketTunnel/WarrenAdapter/WarrenQuinnAdapter.swift`
- Define `WarrenTunnelConfig`, `WarrenTunnelEvent`, `WarrenTunnelStatus` Swift types
- Define empty FFI bindings (stub methods that throw "not implemented")
- Commit + push

### C.4.2 Implement warren_tunnel_ffi.rs Rust side (3-4 days)
- Wire warren-tunnel path-dep into `warren-ios/Cargo.toml`
- Implement `warren_tunnel_start` / `_stop` / `_reconnect` / `_status` /
  `_set_event_callback`
- Add C-repr structs for parameters / status / events
- Run `cargo build --target aarch64-apple-ios` PASS
- cbindgen regenerates `warren_rust_runtime.h` with new exports

### C.4.3 Wire NEPacketTunnelFlow bridge (1-2 days)
- Implement the `readPackets` / `writePackets` <-> Rust channel bridge
  (Swift task spawning + FFI callbacks)
- Validate packet round-trip on iOS Simulator (loopback test)

### C.4.4 Replace WireGuardGoTunnelImplementation (1-2 days)
- Create `WarrenQuinnTunnelImplementation` conforming to `TunnelImplementation`
- Wire it in `PacketTunnelProvider.init()` (replace
  `makeWireGuardGoImplementation()`)
- Drop `WireGuardGoTunnelImplementation.swift` + `WgAdapter.swift`
- Drop WireGuardKit / WireGuardKitTypes refs from pbxproj
- Drop `ios/wireguard-apple/` stub Package.swift (no longer referenced)

### C.4.5 Migrate apiContext + access methods (1-2 days)
- Replace `MullvadApiContext` setup with `WarrenApiClient` setup
- Drop `MullvadAccessMethodReceiver` (Warren M4.0 obfuscation doesn't need it)
- Update `apiTransportProvider` / `REST.ProxyFactory` plumbing
- (Couples with C.3 deep step 2 ; can be done in C.4 if C.3 deep step 2
  not yet started)

### C.4.6 Wire reconnect + path observer (0.5 day)
- Adapt `PacketTunnelPathObserver` to call `adapter.reconnect()` on
  `NWPathMonitor` events

### C.4.7 Wire event broadcasting (0.5 day)
- Implement `broadcastEvent(_:)` in `PacketTunnelProvider`
- Define App Group UserDefaults keys (failover_count, last_failover_at,
  natPmp_externalPort, …)
- App-side UI consumes these in C.6 (multi-hop / DAITA / NAT-PMP UI parity)

### C.4.8 Drop dead WG-PostQuantum code (0.5 day)
- Drop `ios/PacketTunnel/PostQuantum/` (5 .swift files :
  `PacketTunnelActor+PostQuantum.swift`, `EphemeralPeerExchangingPipeline.swift`,
  `MultiHopEphemeralPeerExchanger.swift`, `SingleHopEphemeralPeerExchanger.swift`,
  `PostQuantumKeyExchangeActor.swift`)
- Drop `MullvadPostQuantum+Stubs.swift` from WarrenRustRuntimeTests
- Drop the `talpid-tunnel-config-client` C header from `ios/MullvadPostQuantum/`
- Remove `MullvadPostQuantum` dir entirely

### C.4.9 iOS Simulator smoke test (0.5-1 day)
- Launch app on iPhone 15 simulator
- Connect to warren-exit-1 prod (via dev signing or via no-signing flag)
- Verify : tunnel established, DNS leak test PASS (via `curl ifconfig.me`),
  reconnect on Wi-Fi toggle, multi-hop toggle apply, killswitch active when
  app force-killed

### C.4.10 Tests (1 day)
- Unit tests for `WarrenQuinnAdapter` (mocked FFI, state machine)
- Integration tests for `PacketTunnelProvider` (mocked adapter)
- Tests for App Group event broadcasting (round-trip via UserDefaults)

Total C.4 estimate : 9-13 days. Aligned with brief 10-14 days.

---

## 5. Risks + mitigation

### Risk 1 : utun fd / NEPacketTunnelFlow bridge perf
**Cause** : copying every packet between Swift packetFlow and Rust queue
costs CPU at line rate (>500 Mbps).

**Mitigation** : benchmark early (during C.4.3) with iperf3. If perf
is unacceptable, escalate for a `tun_rs` iOS backend contribution.
Acceptable degradation : 5-10% throughput loss vs raw fd.

### Risk 2 : Background suspension breaking Quinn keep-alive
**Cause** : iOS suspends NEPacketTunnelProvider after ~10 min of
no traffic, even with VPN active.

**Mitigation** : configure Quinn's `keep_alive_interval` to 25s (just
below iOS's typical 30s suspension threshold). Add UDP heartbeat from
warren-tunnel side. Tested upstream Mullvad WireGuardKit uses 25s.

### Risk 3 : NEPacketTunnelProvider memory limit (50 MB)
**Cause** : iOS extensions have a strict 50 MB memory limit. Quinn's
default per-connection memory (recv/send buffers) may be too generous.

**Mitigation** : tune Quinn's transport config :
- `max_concurrent_uni_streams` = 0 (we use bidirectional only)
- `max_concurrent_bidi_streams` = 100
- `receive_window` = 4 MB (conservative)
- `send_window` = 4 MB

Verify memory footprint via Xcode Instruments memory profiler.

### Risk 4 : App Store review reject for VPN extension perms
**Cause** : Apple sometimes rejects VPN apps for missing privacy
manifest / nutrition labels.

**Mitigation** : ensure the App Store metadata in C.7 covers all required
disclosures (cf. brief §C.7.3). Submit privacy nutrition label declaring
network usage + IP address collection (for tunnel, not analytics).

### Risk 5 : Wi-Fi <-> cellular handover dropping connection
**Cause** : Quinn's connection-migration feature requires both sides
to support it. warren-exit needs to accept connection migration packets
from a new client IP. Verify upstream warren-tunnel config.

**Mitigation** : if connection migration is disabled in warren-tunnel,
fall back to reconnect-on-path-change (already wired via
`PacketTunnelPathObserver`).

---

## 6. Out of scope C.4

- C.5 UI wallet wiring (calls into `WarrenQuinnAdapter` happen with a
  config built from settings ; the settings come from C.5 UI)
- C.6 Multi-hop / DAITA / NAT-PMP UI surfacing (the events broadcast
  to App Group are consumed by C.6 banner / status views)
- C.7 TestFlight upload (requires Apple Developer signing)

---

## 7. Open questions for poka

1. **`tun_rs` iOS backend** : contribute upstream or use
   readPackets/writePackets bridge? Recommend bridge for short-term, upstream
   contribution for long-term performance.
2. **Connection migration on warren-exit side** : is it enabled in
   warren-tunnel prod config? If not, ETA for enabling it (Wi-Fi <-> cellular
   handover would benefit significantly).
3. **Apple Developer account TEAM_ID** : needed for the iOS Simulator smoke
   test in C.4.9 only if testing on a real device. Simulator-only smoke does
   not require signing.
4. **NEVPNProtocolWarren** subclass : if we extend `NETunnelProviderProtocol`,
   verify with Apple that custom keys (multi-hop relay, DAITA spec) are
   accepted by App Store review.
5. **WireGuardKit removal** : confirm OK to drop `WireGuardKit` +
   `WireGuardKitTypes` framework refs from pbxproj (cf. ios/wireguard-apple
   stub package). Required for C.4.4.

---

## 8. References

- `warren-tunnel` crate : `warren-core/crates/warren-tunnel/src/lib.rs`
- `warren-multihop` crate : `warren-core/crates/warren-multihop/src/lib.rs`
- `warren-natpmp-client` crate : `warren-core/crates/warren-natpmp-client/src/lib.rs`
- `warren-api-client` crate : `warren-core/crates/warren-api-client/src/lib.rs`
- `warren-identity` crate : `warren-core/crates/warren-identity/src/lib.rs`
- `warren-backoff` crate : `warren-core/crates/warren-backoff/src/lib.rs` (HANDSHAKE backoff = 15s, cf. M4.H.G)
- Session B memory : DAITA v2 multi-conn E2E (`pump_multi_bidirectional_with_daita`), failover (`select_failover_alternative_for_attempt`, `report_exit_down`)
- Session E memory : exit_id stable 16-byte cross-repo (TOFU pubkey pinning A.4)
- Session F memory : `pump_*_with_daita` instable cross-DC sustained (warren-core M5.B.1.X open) — may affect C.4 multi-hop+DAITA testing
- Upstream Mullvad PacketTunnelProvider : `ios/PacketTunnel/PacketTunnelProvider/PacketTunnelProvider.swift`
- Apple NetworkExtension framework documentation : https://developer.apple.com/documentation/networkextension/nepackettunnelprovider
