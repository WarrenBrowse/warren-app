//
//  WarrenQuinnAdapter.swift
//  WarrenRustRuntime
//
//  Created by Warren on 2026-05-21.
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  Swift facade over the `warren_tunnel_ffi` Rust exports
//  (`warren-ios/src/warren_tunnel_ffi.rs`). Implements the contract
//  documented in `.planning/c4-packet-tunnel-provider-quinn-design.md`.
//
//  Bridges `NEPacketTunnelFlow` to the Rust-managed `IosTun` via two
//  channels:
//    - **inbound** : a Swift `Task` loops on `readPackets` and calls
//      `warren_tunnel_inject_inbound_packet` per packet.
//    - **outbound** : a C function pointer registered with
//      `warren_tunnel_set_outbound_callback`. Rust calls it from its
//      dispatcher task ; Swift forwards each packet to
//      `packetFlow.writePackets`.
//
//  The Quinn handshake / pump itself runs Rust-side ; this file is
//  the Swift transport plumbing required for that step.
//

import Foundation
@preconcurrency import NetworkExtension
import WarrenRustRuntimeProxy

/// Configuration passed to `WarrenQuinnAdapter.start(config:)`.
/// Mirrors `warren-tunnel::WarrenTunnelParameters` Rust struct.
public struct WarrenTunnelConfig: Sendable {
    /// 32-byte Ed25519 public key of the exit relay.
    public let exitPubkey: Data
    /// "IP:port" of the exit relay.
    public let exitEndpoint: String
    /// 32-byte Ed25519 signing key derived from the user wallet
    /// (see `WarrenWallet.seed`).
    public let walletSigningKey: Data
    /// Optional multi-hop entry relay configuration. When nil, the tunnel
    /// is single-hop directly to `exitEndpoint`.
    public let multiHopRelay: WarrenRelayConfig?
    /// Optional DAITA defensive shaping spec. When nil, DAITA is off
    /// (the default).
    public let daitaSpec: WarrenDaitaSpec?
    /// Enables NAT-PMP port mapping request through the tunnel after
    /// the Quinn connection is established.
    public let natPmpEnabled: Bool
    /// CIDRs to bypass the tunnel routing (`--bypass-cidr`).
    public let bypassCidrs: [String]
    /// Signed multi-hop directory JSON (fetched from
    /// `GET {api}/v1/multihop/directory`). When non-nil the tunnel rides
    /// the multi-hop wire protocol (the production fleet is multi-hop
    /// only); the FFI verifies it against the baked root pin and selects a
    /// circuit. When nil the FFI falls back to the legacy single-hop path
    /// (dev / loopback only).
    public let multihopDirectoryJSON: String?
    /// `true` selects a 2-hop circuit (entry != exit, country diverse);
    /// `false` a 1-hop circuit collapsed onto one node. Ignored when
    /// `multihopDirectoryJSON` is nil.
    public let multihopTwoHop: Bool
    /// Optional ISO 3166-1 alpha-2 entry-country hint (empty = any).
    public let multihopEntryCountry: String
    /// Optional ISO 3166-1 alpha-2 exit-country hint (empty = any).
    public let multihopExitCountry: String
    /// Path to the App Group file persisting the multi-hop directory
    /// anti-rollback high-water mark. When nil the FFI keeps the rollback
    /// gate per-connect only (no cross-connect persistence).
    public let multihopGenerationStatePath: String?
    /// Path to the App Group file persisting the exit-pubkey TOFU pin
    /// table. When non-nil the FFI enforces the pin and fails the
    /// connection closed on a mismatch (the mismatch is then queryable via
    /// [`WarrenQuinnAdapter.takePinMismatch`]). When nil pinning is off.
    public let pinStorePath: String?

    public init(
        exitPubkey: Data,
        exitEndpoint: String,
        walletSigningKey: Data,
        multiHopRelay: WarrenRelayConfig? = nil,
        daitaSpec: WarrenDaitaSpec? = nil,
        natPmpEnabled: Bool = false,
        bypassCidrs: [String] = [],
        multihopDirectoryJSON: String? = nil,
        multihopTwoHop: Bool = false,
        multihopEntryCountry: String = "",
        multihopExitCountry: String = "",
        multihopGenerationStatePath: String? = nil,
        pinStorePath: String? = nil
    ) {
        self.exitPubkey = exitPubkey
        self.exitEndpoint = exitEndpoint
        self.walletSigningKey = walletSigningKey
        self.multiHopRelay = multiHopRelay
        self.daitaSpec = daitaSpec
        self.natPmpEnabled = natPmpEnabled
        self.bypassCidrs = bypassCidrs
        self.multihopDirectoryJSON = multihopDirectoryJSON
        self.multihopTwoHop = multihopTwoHop
        self.multihopEntryCountry = multihopEntryCountry
        self.multihopExitCountry = multihopExitCountry
        self.multihopGenerationStatePath = multihopGenerationStatePath
        self.pinStorePath = pinStorePath
    }
}

/// Details of the last exit-pubkey TOFU mismatch recorded by the tunnel.
/// Decoded from the JSON returned by `warren_tunnel_take_pin_mismatch`.
/// Mirrors the desktop `WarrenPubkeyMismatch` daemon-rpc type. All hex
/// fields are lowercase 64-char Ed25519 representations.
public struct WarrenPinMismatch: Codable, Equatable, Sendable {
    /// Hex exit identifier the mismatch was observed for.
    public let exitId: String
    /// Hex of the pubkey the exit served on this connect (the new key).
    public let observed: String
    /// Hex of the pubkey previously pinned for `exitId`.
    public let pinned: String
    /// ISO 3166-1 alpha-2 country code of the exit, or "" when unknown.
    public let country: String

    // The Rust FFI emits snake_case keys (`exit_id`); the other three
    // already match. Encoding round-trips through the same keys so the
    // App Group payload the extension writes decodes in the main app.
    private enum CodingKeys: String, CodingKey {
        case exitId = "exit_id"
        case observed
        case pinned
        case country
    }

    public init(exitId: String, observed: String, pinned: String, country: String) {
        self.exitId = exitId
        self.observed = observed
        self.pinned = pinned
        self.country = country
    }
}

/// Multi-hop entry relay config. Used when the user wants traffic
/// routed through an entry relay before reaching the exit
/// (HPKE handshake initiated by `warren-multihop::MultiHopClient`).
public struct WarrenRelayConfig: Sendable {
    public let pubkey: Data  // 32 bytes Ed25519
    public let endpoint: String  // "IP:port"
    public let countryCode: String  // ISO 3166-1 alpha-2, e.g. "SE"

    public init(pubkey: Data, endpoint: String, countryCode: String) {
        self.pubkey = pubkey
        self.endpoint = endpoint
        self.countryCode = countryCode
    }
}

/// DAITA defensive shaping spec carried in `SetupAck.daita_spec`.
public struct WarrenDaitaSpec: Sendable {
    public let machineSeedHex: String  // 32-byte hex Maybenot machine seed
    public let padding: UInt32  // padding budget in packets/sec

    public init(machineSeedHex: String, padding: UInt32) {
        self.machineSeedHex = machineSeedHex
        self.padding = padding
    }
}

/// Status reported by `WarrenQuinnAdapter.status()`.
public struct WarrenTunnelStatus: Sendable {
    public enum State: Sendable {
        case disconnected
        case connecting
        case connected
        case reconnecting
        case failed(String)
    }
    public let state: State
    public let bytesIn: UInt64
    public let bytesOut: UInt64
    /// Seconds since the current connection was established. `nil` when
    /// `state != .connected`.
    public let connectedDurationSeconds: UInt64?
    /// Cumulative failover count this session.
    public let failoverCount: UInt32
}

/// Events broadcast by the adapter through the user-supplied callback.
/// Consumers should mirror these into App Group `UserDefaults` so the
/// main app can react (e.g. failover banner, NAT-PMP port display).
public enum WarrenTunnelEvent: Sendable {
    case connected
    case disconnected
    case reconnecting
    case failover(toExit: String)
    case natPmpMapped(internalPort: UInt16, externalPort: UInt16, lifetime: UInt32)
    case natPmpRenewed(externalPort: UInt16)
    case natPmpFailed(reason: String)
}

/// Exit-allocated IPv4 surfaced by the multi-hop circuit after its
/// setup-stream returns an `IpAssign`. The consumer re-applies the tunnel
/// network settings so the TUN source IP matches what the exit expects
/// (the iOS analog of the daemon's `RealTun::reassign_ipv4`).
public struct WarrenTunnelIpAssign: Sendable {
    /// Dotted-decimal exit-allocated IPv4 (e.g. "10.66.0.2").
    public let ipv4: String
    /// Subnet prefix length for the allocated address.
    public let prefixLength: Int
    /// Dotted-decimal exit-side gateway IPv4.
    public let gatewayIPv4: String
}

/// Errors emitted by `WarrenQuinnAdapter`.
public enum WarrenQuinnAdapterError: Error {
    /// The adapter was never started (`start(config:)` not called).
    case notStarted
    /// `start(config:)` was called twice without intervening `stop()`.
    case alreadyStarted
    /// FFI `warren_tunnel_start` returned a null handle (runtime alloc
    /// failure, invalid parameters, or `tunnel` feature disabled at
    /// build time).
    case ffiStartFailed
    /// FFI returned a non-zero return code from one of the entry
    /// points. Wraps the raw `c_int` for diagnostics.
    case ffi(Int32)
    /// A fixed-size key field had the wrong length. Marshalling refuses to
    /// silently truncate or zero-pad crypto key material, which would
    /// produce a wrong identity with no error.
    case invalidKeyLength(field: String, expected: Int, actual: Int)
}

/// High-level Swift facade over the warren-tunnel / warren-ios FFI.
///
/// Replaces the upstream Mullvad `WgAdapter` (WireGuardKit wrapper)
/// with a Quinn-based implementation backed by `warren-tunnel`.
///
/// The adapter is a `final class` (not an `actor`) because the Rust
/// outbound callback fires from a non-Swift thread and would otherwise
/// require an `await` hop per packet ; the synchronous class lets us
/// forward outbound packets to `packetFlow.writePackets` immediately,
/// from any thread (`NEPacketTunnelFlow.writePackets` is thread-safe).
/// Internal mutable state is guarded by an `NSLock`.
/// The slice of `WarrenQuinnAdapter` that `WarrenQuinnActor` drives.
/// Extracted as a protocol so the actor can be unit-tested against a mock
/// without standing up the real Rust FFI tunnel.
public protocol WarrenQuinnAdapting: AnyObject {
    func start(config: WarrenTunnelConfig) throws
    func stop()
    func reconnect()
    func pause()
    func resume()
}

public final class WarrenQuinnAdapter: @unchecked Sendable, WarrenQuinnAdapting {
    // `fileprivate` (not `private`) so the file-level @convention(c)
    // callbacks below can access these via the Unmanaged self-ref
    // recovered from the FFI context pointer.
    fileprivate let packetFlow: NEPacketTunnelFlow
    fileprivate let eventCallback: @Sendable (WarrenTunnelEvent) -> Void
    /// Fired when the multi-hop circuit reports a fresh exit-allocated
    /// IPv4. Nil on the single-hop dev path (and in unit tests), where no
    /// reassign ever happens.
    fileprivate let ipAssignCallback: (@Sendable (WarrenTunnelIpAssign) -> Void)?

    private let lock = NSLock()
    private var handle: OpaquePointer?
    private var inboundTask: Task<Void, Never>?
    /// Retained `Unmanaged` reference used as the FFI callback context.
    /// Released on `stop()` so the adapter can be deinit-ed.
    private var ffiContextRetain: Unmanaged<WarrenQuinnAdapter>?

    public init(
        packetFlow: NEPacketTunnelFlow,
        eventCallback: @escaping @Sendable (WarrenTunnelEvent) -> Void,
        ipAssignCallback: (@Sendable (WarrenTunnelIpAssign) -> Void)? = nil
    ) {
        self.packetFlow = packetFlow
        self.eventCallback = eventCallback
        self.ipAssignCallback = ipAssignCallback
    }

    deinit {
        // Defensive: if `stop()` was never called, release the FFI
        // handle here. The retained `Unmanaged` self-reference is
        // released by `stop()` ; if we hit this path it means a leak
        // already happened upstream, so just clean what we can.
        if let h = handle {
            warren_tunnel_stop(rawTunnelHandle(h))
        }
    }

    /// Starts the Warren Quinn tunnel with the given configuration.
    /// Marshals `config` to the FFI struct, registers the outbound
    /// callback (Rust → `packetFlow.writePackets`) and starts the
    /// inbound `readPackets` loop.
    ///
    /// - Throws: `WarrenQuinnAdapterError.alreadyStarted` if a previous
    ///   `start(config:)` is still active (call `stop()` first), or
    ///   `WarrenQuinnAdapterError.ffiStartFailed` if `warren_tunnel_start`
    ///   returns null (runtime alloc failure or `tunnel` feature off).
    public func start(config: WarrenTunnelConfig) throws {
        lock.lock()
        defer { lock.unlock() }
        guard handle == nil else { throw WarrenQuinnAdapterError.alreadyStarted }

        let newHandle = try Self.callTunnelStart(config: config)
        let retainedSelf = Unmanaged.passRetained(self)

        let outboundStatus = warren_tunnel_set_outbound_callback(
            tunnelHandle(newHandle),
            outboundCallback,
            retainedSelf.toOpaque()
        )
        guard outboundStatus == 0 else {
            warren_tunnel_stop(tunnelHandle(newHandle))
            retainedSelf.release()
            throw WarrenQuinnAdapterError.ffi(outboundStatus)
        }

        // Same retained self-pointer is used as the event-callback
        // context. The two callbacks share lifecycle ; both are
        // dropped together when the Rust-side Box is released by
        // `warren_tunnel_stop`.
        let eventStatus = warren_tunnel_set_event_callback(
            tunnelHandle(newHandle),
            eventCallbackBridge,
            retainedSelf.toOpaque()
        )
        guard eventStatus == 0 else {
            warren_tunnel_stop(tunnelHandle(newHandle))
            retainedSelf.release()
            throw WarrenQuinnAdapterError.ffi(eventStatus)
        }

        // Exit-allocated IP callback. Registered only when a consumer
        // wants reassign notifications (the multi-hop path). The bridge
        // recovers the same retained self-ref; a missing closure is a
        // no-op inside the bridge, so an FFI failure here is non-fatal.
        if ipAssignCallback != nil {
            let ipAssignStatus = warren_tunnel_set_ip_assign_callback(
                tunnelHandle(newHandle),
                ipAssignCallbackBridge,
                retainedSelf.toOpaque()
            )
            guard ipAssignStatus == 0 else {
                warren_tunnel_stop(tunnelHandle(newHandle))
                retainedSelf.release()
                throw WarrenQuinnAdapterError.ffi(ipAssignStatus)
            }
        }

        handle = newHandle
        ffiContextRetain = retainedSelf
        inboundTask = Task { [weak self] in
            await self?.inboundPumpLoop(handle: newHandle)
        }
    }

    /// Stops the tunnel and releases all resources. Idempotent.
    public func stop() {
        lock.lock()
        let oldHandle = handle
        let oldTask = inboundTask
        let oldRetain = ffiContextRetain
        handle = nil
        inboundTask = nil
        ffiContextRetain = nil
        lock.unlock()

        oldTask?.cancel()
        if let h = oldHandle {
            // `warren_tunnel_stop` drops the Rust-side handle which
            // includes the outbound dispatcher task ; no late call
            // into the C callback can fire after this point because
            // the dispatcher's mpsc receiver gets dropped.
            warren_tunnel_stop(rawTunnelHandle(h))
        }
        oldRetain?.release()
    }

    /// Triggers a tunnel reconnect (e.g. on Wi-Fi <-> cellular handover).
    /// Returns silently when the tunnel is currently disconnected
    /// (reconnect makes no sense in that state).
    public func reconnect() {
        lock.lock()
        let h = handle
        lock.unlock()
        guard let h else { return }
        _ = warren_tunnel_reconnect(rawTunnelHandle(h))
    }

    /// Pauses the inbound pump without tearing down the Quinn
    /// connection. Use on iOS `sleep()` to comply with NetworkExtension
    /// background time budgets. Resume via [`resume()`]. Idempotent.
    public func pause() {
        lock.lock()
        let h = handle
        lock.unlock()
        guard let h else { return }
        _ = warren_tunnel_pause(rawTunnelHandle(h))
    }

    /// Resumes the inbound pump after [`pause()`]. Idempotent.
    public func resume() {
        lock.lock()
        let h = handle
        lock.unlock()
        guard let h else { return }
        _ = warren_tunnel_resume(rawTunnelHandle(h))
    }

    /// Returns the current tunnel status. Reads from atomic counters
    /// on the Rust side ; safe to call from any thread.
    public func status() -> WarrenTunnelStatus {
        lock.lock()
        let h = handle
        lock.unlock()

        var statusC = WarrenTunnelStatusC(
            state: Disconnected,
            bytes_in: 0,
            bytes_out: 0,
            connected_duration_seconds: 0,
            failover_count: 0
        )
        if let h {
            _ = warren_tunnel_status(rawTunnelHandle(h), &statusC)
        }
        let mappedState: WarrenTunnelStatus.State
        switch statusC.state {
        case Connecting: mappedState = .connecting
        case Connected: mappedState = .connected
        case Reconnecting: mappedState = .reconnecting
        case Failed: mappedState = .failed("ffi-reported failure")
        default: mappedState = .disconnected
        }
        let connectedDuration: UInt64? =
            statusC.state == Connected && statusC.connected_duration_seconds > 0
            ? statusC.connected_duration_seconds : nil
        return WarrenTunnelStatus(
            state: mappedState,
            bytesIn: statusC.bytes_in,
            bytesOut: statusC.bytes_out,
            connectedDurationSeconds: connectedDuration,
            failoverCount: statusC.failover_count
        )
    }

    /// Take (and clear) the details of the last exit-pubkey TOFU mismatch
    /// recorded on the live handle, or nil when none is pending. A TOFU
    /// mismatch surfaces as a connection failure (the Rust side fails
    /// closed), so the consumer calls this on a failed connect to decide
    /// whether to present the Trust / Report / Reject alert.
    ///
    /// Returns nil when the tunnel was never started, when there is no
    /// pending mismatch, or when the JSON cannot be decoded.
    public func takePinMismatch() -> WarrenPinMismatch? {
        lock.lock()
        let h = handle
        lock.unlock()
        guard let h else { return nil }
        guard let raw = warren_tunnel_take_pin_mismatch(rawTunnelHandle(h)) else { return nil }
        // The returned heap string is freed via the type-agnostic free
        // routine the account/wallet FFIs use for their C strings.
        defer { warren_wallet_free_mnemonic(raw) }
        let json = String(cString: raw)
        guard let data = json.data(using: .utf8) else { return nil }
        return try? JSONDecoder().decode(WarrenPinMismatch.self, from: data)
    }

    // MARK: - Internal: inbound pump

    /// Read packets from `NEPacketTunnelFlow` in a loop and push each
    /// one into the Rust-side `IosTun` inbound queue.
    ///
    /// `NEPacketTunnelFlow.readPackets` returns a batch ; we forward
    /// each packet one by one. The loop exits when `inboundTask` is
    /// cancelled (handle reset on `stop()`).
    private func inboundPumpLoop(handle: OpaquePointer) async {
        while !Task.isCancelled {
            let packets: [Data] = await withCheckedContinuation { cont in
                packetFlow.readPackets { packets, _protocols in
                    cont.resume(returning: packets)
                }
            }
            if packets.isEmpty {
                // `readPackets` returns synchronously on flow shutdown
                // with an empty batch ; bail out to avoid a tight loop.
                break
            }
            for packet in packets {
                _ = packet.withUnsafeBytes { bytes -> Int32 in
                    guard let base = bytes.baseAddress else { return 0 }
                    return warren_tunnel_inject_inbound_packet(
                        rawTunnelHandle(handle),
                        base.assumingMemoryBound(to: UInt8.self),
                        UInt(packet.count)
                    )
                }
            }
        }
    }

    // MARK: - Internal: marshalling

    /// Returns `data` as a `count`-byte array, throwing
    /// `invalidKeyLength` if the length differs. Crypto key material must
    /// not be silently truncated or zero-padded: a wrong-length key would
    /// otherwise yield a silently wrong identity instead of a clear error.
    private static func fixedKeyBytes(_ data: Data, count: Int, field: String) throws -> [UInt8] {
        guard data.count == count else {
            throw WarrenQuinnAdapterError.invalidKeyLength(
                field: field, expected: count, actual: data.count
            )
        }
        return [UInt8](data)
    }

    /// Marshal a Swift `WarrenTunnelConfig` to its C representation
    /// and call `warren_tunnel_start`. All pointers in the C struct
    /// are valid only for the duration of the call ; the FFI must
    /// copy any field it needs to retain.
    ///
    /// Marshalling layers, outermost first :
    /// 1. exit endpoint + bypass CIDRs (always pinned).
    /// 2. optional multi-hop relay endpoint + country code (pinned
    ///    inside an extra `withCString` pair when `config.multiHopRelay`
    ///    is non-nil).
    /// 3. optional DAITA machine seed (decoded from hex, by value, no
    ///    pinning required - tuple-by-value in the C struct).
    /// 4. `withUnsafePointer(to:)` to expose `&relayCfg` / `&daitaCfg`
    ///    as stable C pointers inside the innermost FFI call.
    ///
    /// 5. multi-hop directory JSON (optional) + entry/exit country hints
    ///    (innermost pin). When the directory is present the Rust side
    ///    verifies it and rides the multi-hop wire protocol; the legacy
    ///    `multiHopRelay` field is unused on that path.
    fileprivate static func callTunnelStart(config: WarrenTunnelConfig) throws -> OpaquePointer
    {
        let exitPubkeyBytes = try Self.fixedKeyBytes(
            config.exitPubkey, count: 32, field: "exitPubkey"
        )
        var signingSeedBytes = try Self.fixedKeyBytes(
            config.walletSigningKey, count: 32, field: "walletSigningKey"
        )
        defer {
            // Wipe sensitive material before returning. The FFI has
            // copied anything it needs (the Rust struct holds the seed
            // by value, not by pointer).
            for i in 0..<signingSeedBytes.count { signingSeedBytes[i] = 0 }
        }

        let bypass = config.bypassCidrs
        let natPmpFlag: UInt8 = config.natPmpEnabled ? 1 : 0
        let exitPubkeyTuple = tupleFrom32(exitPubkeyBytes)
        let signingSeedTuple = tupleFrom32(signingSeedBytes)

        // Pre-compute the DAITA bytes-tuple if present. The C struct
        // takes it by value so no pinning is needed below.
        let daitaTuple: (UInt32, (UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8,
            UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8,
            UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8,
            UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8))? = config.daitaSpec.map
        { spec in
            var seed = [UInt8](repeating: 0, count: 32)
            if let raw = Data(hexString: spec.machineSeedHex) {
                raw.copyBytes(to: &seed, count: min(32, raw.count))
            }
            return (spec.padding, tupleFrom32(seed))
        }

        // Multi-hop pubkey tuple is pre-computed too ; only the strings
        // need a `withCString` pin.
        let relayPubkeyTuple: (UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8,
            UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8,
            UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8,
            UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8)? = try config.multiHopRelay
            .map { relay in
                let bytes = try Self.fixedKeyBytes(
                    relay.pubkey, count: 32, field: "multiHopRelay.pubkey"
                )
                return tupleFrom32(bytes)
            }

        // Outer-most pin : exit endpoint + bypass CIDRs.
        let handlePtr: UnsafeMutablePointer<WarrenTunnelHandle>? =
            config.exitEndpoint.withCString { exitEndpointC in
                withCStrings(bypass) { bypassArray in
                    let bypassCount = UInt32(bypass.count)
                    let bypassBase = bypassArray.baseAddress
                    // Inner pin : multi-hop relay strings (when present),
                    // then DAITA-by-value, then build parameters + FFI.
                    return Self.withMultiHopRelayPinned(config.multiHopRelay, pubkeyTuple: relayPubkeyTuple) {
                        relayPtr in
                        Self.withDaitaPinned(daitaTuple) { daitaPtr in
                            let twoHopFlag: UInt8 = config.multihopTwoHop ? 1 : 0
                            // Innermost pin: the multi-hop directory JSON
                            // (optional) + entry/exit country hints.
                            return config.multihopEntryCountry.withCString { entryC in
                                config.multihopExitCountry.withCString { exitC in
                                    Self.withOptionalCString(config.multihopDirectoryJSON) { dirPtr in
                                        Self.withOptionalCString(config.multihopGenerationStatePath) { genPtr in
                                            Self.withOptionalCString(config.pinStorePath) { pinPtr in
                                                var parameters = WarrenTunnelParametersC(
                                                    exit_pubkey: exitPubkeyTuple,
                                                    exit_endpoint: exitEndpointC,
                                                    wallet_signing_seed: signingSeedTuple,
                                                    multi_hop_relay: relayPtr,
                                                    daita_spec: daitaPtr,
                                                    nat_pmp_enabled: natPmpFlag,
                                                    bypass_cidrs: bypassBase,
                                                    bypass_cidrs_count: bypassCount,
                                                    multihop_directory_json: dirPtr,
                                                    multihop_two_hop: twoHopFlag,
                                                    multihop_entry_country: entryC,
                                                    multihop_exit_country: exitC,
                                                    multihop_generation_state_path: genPtr,
                                                    pin_store_path: pinPtr
                                                )
                                                return warren_tunnel_start(&parameters, -1)
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

        guard let raw = handlePtr else { throw WarrenQuinnAdapterError.ffiStartFailed }
        return OpaquePointer(raw)
    }

    /// Verify a freshly fetched multi-hop directory and return its trusted
    /// `generation`, or nil on any verification / expiry / rollback failure.
    /// Handle-free: the periodic refresh uses it to detect a fleet change (a
    /// higher generation than the running session's) without disturbing the
    /// live tunnel. Does not raise the persisted high-water mark.
    public static func checkMultihopGeneration(
        directoryJSON: String,
        generationStatePath: String?
    ) -> Int64? {
        let result = directoryJSON.withCString { jsonC in
            withOptionalCString(generationStatePath) { pathC in
                warren_multihop_check_generation(jsonC, pathC)
            }
        }
        return result >= 0 ? result : nil
    }

    /// Trust a (possibly new) exit pubkey for `exitIdHex`, overwriting any
    /// existing pin in the App Group store at `pinStorePath`. Called when
    /// the user accepts a TOFU mismatch ("Trust new key"). Handle-free: it
    /// mutates the on-disk store, not the live session. Returns true on
    /// success, false on invalid input.
    public static func pinTrust(
        pinStorePath: String,
        exitIdHex: String,
        pubkeyHex: String,
        country: String
    ) -> Bool {
        let result = pinStorePath.withCString { pathC in
            exitIdHex.withCString { exitC in
                pubkeyHex.withCString { pubkeyC in
                    country.withCString { countryC in
                        warren_pin_trust(pathC, exitC, pubkeyC, countryC)
                    }
                }
            }
        }
        return result == 0
    }

    /// Clear all exit-pubkey pins in the App Group store at
    /// `pinStorePath`. Backs the Settings "Reset pinned exit keys" action.
    /// Returns the number of pins dropped (>= 0), or -1 on invalid input.
    public static func pinReset(pinStorePath: String) -> Int {
        let result = pinStorePath.withCString { pathC in
            warren_pin_reset(pathC)
        }
        return Int(result)
    }

    /// Pin an optional Swift string as a C string for the duration of
    /// `body`, passing a null pointer when the string is nil.
    private static func withOptionalCString<Result>(
        _ string: String?,
        _ body: (UnsafePointer<CChar>?) -> Result
    ) -> Result {
        if let string {
            return string.withCString { body($0) }
        }
        return body(nil)
    }

    /// Pin a `WarrenRelayConfigC` C struct on the stack for the
    /// duration of `body`. When `relay` is nil, calls `body(nil)`
    /// without allocating anything.
    ///
    /// The endpoint + country code strings are pinned via nested
    /// `withCString` blocks ; the resulting pointers are written into
    /// the local `WarrenRelayConfigC` immediately before exposing the
    /// pointer via `withUnsafePointer(to:)`.
    private static func withMultiHopRelayPinned<Result>(
        _ relay: WarrenRelayConfig?,
        pubkeyTuple: (
            UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8,
            UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8,
            UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8,
            UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8
        )?,
        _ body: (UnsafePointer<WarrenRelayConfigC>?) -> Result
    ) -> Result {
        guard let relay, let pubkeyTuple else {
            return body(nil)
        }
        return relay.endpoint.withCString { endpointC in
            relay.countryCode.withCString { countryC in
                var relayCfg = WarrenRelayConfigC(
                    pubkey: pubkeyTuple,
                    endpoint: endpointC,
                    country_code: countryC
                )
                return withUnsafePointer(to: &relayCfg) { ptr in
                    body(ptr)
                }
            }
        }
    }

    /// Pin a `WarrenDaitaSpecC` C struct on the stack for the duration
    /// of `body`. When `daita` is nil, calls `body(nil)`.
    ///
    /// `daita` is the pre-computed `(padding_pps, machine_seed_tuple)`
    /// pair produced by [`callTunnelStart`]. No string pinning is
    /// needed - the seed is a fixed-size byte tuple.
    private static func withDaitaPinned<Result>(
        _ daita: (
            UInt32,
            (UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8,
                UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8,
                UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8,
                UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8)
        )?,
        _ body: (UnsafePointer<WarrenDaitaSpecC>?) -> Result
    ) -> Result {
        guard let daita else { return body(nil) }
        var daitaCfg = WarrenDaitaSpecC(machine_seed: daita.1, padding_pps: daita.0)
        return withUnsafePointer(to: &daitaCfg) { ptr in
            body(ptr)
        }
    }
}

// MARK: - C function pointers + FFI bridging helpers

/// Rust → Swift outbound packet callback. Invoked from a Tokio task on
/// the warren-tunnel runtime ; forwards each packet to
/// `NEPacketTunnelFlow.writePackets`.
///
/// The function is `@convention(c)` so it can be passed as a raw C
/// function pointer to `warren_tunnel_set_outbound_callback`. The
/// `context` pointer is the `Unmanaged` self-reference set at
/// registration time.
private let outboundCallback:
    @convention(c) (UnsafePointer<UInt8>?, UInt, UnsafeMutableRawPointer?) -> Void = {
        dataPtr, len, contextPtr in
        guard let dataPtr, let contextPtr, len > 0 else { return }
        let adapter = Unmanaged<WarrenQuinnAdapter>.fromOpaque(contextPtr).takeUnretainedValue()
        let buf = UnsafeBufferPointer(start: dataPtr, count: Int(len))
        let packet = Data(buf)
        // Default IPv4 protocol number ; iOS auto-detects on writePackets
        // if the packet is IPv6 but we still need to pass *something*.
        // Inspect the first nibble of the header to disambiguate.
        let proto: NSNumber = (packet.first.map { ($0 >> 4) == 6 } ?? false)
            ? NSNumber(value: AF_INET6) : NSNumber(value: AF_INET)
        adapter.packetFlow.writePackets([packet], withProtocols: [proto])
    }

/// Rust → Swift event callback. Invoked from the warren-tunnel
/// dispatcher whenever the connection state changes or a NAT-PMP
/// event fires. Marshals the C tagged-union into the Swift
/// [`WarrenTunnelEvent`] enum and forwards to the user-supplied
/// closure.
///
/// `event` is owned by Rust for the duration of the call ; we copy
/// every UTF-8 string we keep (country code, failure reason). The
/// adapter's `eventCallback` closure is `@Sendable` so callers can
/// post to any actor / queue.
private let eventCallbackBridge:
    @convention(c) (UnsafePointer<WarrenTunnelEventC>?, UnsafeMutableRawPointer?) -> Void = {
        eventPtr, contextPtr in
        guard let eventPtr, let contextPtr else { return }
        let adapter = Unmanaged<WarrenQuinnAdapter>.fromOpaque(contextPtr).takeUnretainedValue()
        let event = eventPtr.pointee
        let mapped: WarrenTunnelEvent
        switch event.tag {
        case EventConnected: mapped = .connected
        case EventDisconnected: mapped = .disconnected
        case EventReconnecting: mapped = .reconnecting
        case EventFailover:
            let country = event.data_failover_country_code.flatMap { String(cString: $0) } ?? ""
            mapped = .failover(toExit: country)
        case EventNatPmpMapped:
            mapped = .natPmpMapped(
                internalPort: event.data_nat_pmp_internal_port,
                externalPort: event.data_nat_pmp_external_port,
                lifetime: event.data_nat_pmp_lifetime_seconds
            )
        case EventNatPmpRenewed:
            mapped = .natPmpRenewed(externalPort: event.data_nat_pmp_external_port)
        case EventNatPmpFailed:
            let reason = event.data_nat_pmp_failure_reason.flatMap { String(cString: $0) } ?? ""
            mapped = .natPmpFailed(reason: reason)
        default:
            return
        }
        adapter.eventCallback(mapped)
    }

/// Rust → Swift exit-allocated IP callback. Invoked from the multi-hop
/// reassign task when the circuit reports a fresh `IpAssign`. Marshals
/// the C struct into [`WarrenTunnelIpAssign`] and forwards to the
/// user-supplied closure (which re-applies the tunnel network settings).
///
/// `assign` is owned by Rust for the duration of the call; the octet
/// tuples are copied by value into dotted-decimal strings.
private let ipAssignCallbackBridge:
    @convention(c) (UnsafePointer<WarrenTunnelIpAssignC>?, UnsafeMutableRawPointer?) -> Void = {
        assignPtr, contextPtr in
        guard let assignPtr, let contextPtr else { return }
        let adapter = Unmanaged<WarrenQuinnAdapter>.fromOpaque(contextPtr).takeUnretainedValue()
        guard let callback = adapter.ipAssignCallback else { return }
        let assign = assignPtr.pointee
        let mapped = WarrenTunnelIpAssign(
            ipv4: dottedIPv4(assign.ipv4),
            prefixLength: Int(assign.prefix_len),
            gatewayIPv4: dottedIPv4(assign.gateway_ipv4)
        )
        callback(mapped)
    }

/// Format a cbindgen `[u8; 4]` tuple as a dotted-decimal IPv4 string.
private func dottedIPv4(_ octets: (UInt8, UInt8, UInt8, UInt8)) -> String {
    "\(octets.0).\(octets.1).\(octets.2).\(octets.3)"
}

/// Cast our typed `OpaquePointer` handle back to the C-bindgen pointer
/// type. Inverses [`rawTunnelHandle`].
private func tunnelHandle(_ ptr: OpaquePointer) -> UnsafeMutablePointer<WarrenTunnelHandle> {
    UnsafeMutablePointer<WarrenTunnelHandle>(ptr)
}

/// Same as [`tunnelHandle`] but accepts a non-mutable `OpaquePointer`
/// (the `stop` / `status` / `reconnect` paths). The Rust side does
/// not distinguish mutability so both casts produce identical raw
/// pointers ; the second helper just keeps the call sites readable.
private func rawTunnelHandle(_ ptr: OpaquePointer) -> UnsafeMutablePointer<WarrenTunnelHandle> {
    UnsafeMutablePointer<WarrenTunnelHandle>(ptr)
}

/// Convert a `[UInt8]` of length 32 to the fixed-size tuple cbindgen
/// emits for `[u8; 32]` C-array fields. cbindgen does not provide an
/// idiomatic Swift helper, so we expand the array manually.
private func tupleFrom32(_ array: [UInt8]) -> (
    UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8,
    UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8,
    UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8,
    UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8
) {
    var a = array
    while a.count < 32 { a.append(0) }
    return (
        a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7],
        a[8], a[9], a[10], a[11], a[12], a[13], a[14], a[15],
        a[16], a[17], a[18], a[19], a[20], a[21], a[22], a[23],
        a[24], a[25], a[26], a[27], a[28], a[29], a[30], a[31]
    )
}

/// Pin an array of Swift strings as a contiguous `[const char *]`
/// array for the duration of `body`. The C-string buffers are owned
/// by the inner `withCString` calls ; the outer pointer array is
/// stack-allocated.
private func withCStrings<Result>(
    _ strings: [String],
    _ body: (UnsafeBufferPointer<UnsafePointer<CChar>?>) -> Result
) -> Result {
    if strings.isEmpty {
        let empty: UnsafeBufferPointer<UnsafePointer<CChar>?> = UnsafeBufferPointer(
            start: nil,
            count: 0
        )
        return body(empty)
    }
    // Nested withCString accumulator pattern (depth = strings.count).
    func recurse(
        _ index: Int,
        _ accumulated: [UnsafePointer<CChar>?]
    ) -> Result {
        if index >= strings.count {
            return accumulated.withUnsafeBufferPointer { body($0) }
        }
        return strings[index].withCString { cstr in
            var next = accumulated
            next.append(cstr)
            return recurse(index + 1, next)
        }
    }
    return recurse(0, [])
}

// MARK: - Helpers

extension Data {
    /// Decode a hex string into bytes. Returns nil on invalid input.
    fileprivate init?(hexString: String) {
        let cleaned = hexString.replacingOccurrences(of: " ", with: "")
        guard cleaned.count.isMultiple(of: 2) else { return nil }
        var bytes = [UInt8]()
        bytes.reserveCapacity(cleaned.count / 2)
        var index = cleaned.startIndex
        while index < cleaned.endIndex {
            let next = cleaned.index(index, offsetBy: 2)
            guard let byte = UInt8(cleaned[index..<next], radix: 16) else { return nil }
            bytes.append(byte)
            index = next
        }
        self.init(bytes)
    }
}

