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
//  The Quinn handshake / pump itself is wired in C.4.1 ; this file is
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
    /// (default per memory `warren_daita_doctrine_v1`).
    public let daitaSpec: WarrenDaitaSpec?
    /// Enables NAT-PMP port mapping request through the tunnel after
    /// the Quinn connection is established (M4.H.F differentiator).
    public let natPmpEnabled: Bool
    /// CIDRs to bypass the tunnel routing (M4.H.G `--bypass-cidr`).
    public let bypassCidrs: [String]

    public init(
        exitPubkey: Data,
        exitEndpoint: String,
        walletSigningKey: Data,
        multiHopRelay: WarrenRelayConfig? = nil,
        daitaSpec: WarrenDaitaSpec? = nil,
        natPmpEnabled: Bool = false,
        bypassCidrs: [String] = []
    ) {
        self.exitPubkey = exitPubkey
        self.exitEndpoint = exitEndpoint
        self.walletSigningKey = walletSigningKey
        self.multiHopRelay = multiHopRelay
        self.daitaSpec = daitaSpec
        self.natPmpEnabled = natPmpEnabled
        self.bypassCidrs = bypassCidrs
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

/// DAITA defensive shaping spec carried in `SetupAck.daita_spec`
/// (cf. memory `warren_session_b_delivered` M5.B.1).
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
    /// Cumulative failover count this session (cf. M5.B.2).
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
public final class WarrenQuinnAdapter: @unchecked Sendable {
    // `fileprivate` (not `private`) so the file-level @convention(c)
    // callbacks below can access these via the Unmanaged self-ref
    // recovered from the FFI context pointer.
    fileprivate let packetFlow: NEPacketTunnelFlow
    fileprivate let eventCallback: @Sendable (WarrenTunnelEvent) -> Void

    private let lock = NSLock()
    private var handle: OpaquePointer?
    private var inboundTask: Task<Void, Never>?
    /// Retained `Unmanaged` reference used as the FFI callback context.
    /// Released on `stop()` so the adapter can be deinit-ed.
    private var ffiContextRetain: Unmanaged<WarrenQuinnAdapter>?

    public init(
        packetFlow: NEPacketTunnelFlow,
        eventCallback: @escaping @Sendable (WarrenTunnelEvent) -> Void
    ) {
        self.packetFlow = packetFlow
        self.eventCallback = eventCallback
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
    /// The Rust side currently ignores both fields (C.4.1 wires them
    /// into `warren_tunnel::ClientTunnel::connect`) - but Swift-side
    /// marshalling is in place so the future wire-up is a one-line
    /// flip on the Rust side.
    fileprivate static func callTunnelStart(config: WarrenTunnelConfig) throws -> OpaquePointer
    {
        var exitPubkeyBytes = [UInt8](repeating: 0, count: 32)
        config.exitPubkey.copyBytes(to: &exitPubkeyBytes, count: min(32, config.exitPubkey.count))
        var signingSeedBytes = [UInt8](repeating: 0, count: 32)
        config.walletSigningKey.copyBytes(
            to: &signingSeedBytes,
            count: min(32, config.walletSigningKey.count)
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
            UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8)? = config.multiHopRelay
            .map { relay in
                var bytes = [UInt8](repeating: 0, count: 32)
                relay.pubkey.copyBytes(to: &bytes, count: min(32, relay.pubkey.count))
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
                            var parameters = WarrenTunnelParametersC(
                                exit_pubkey: exitPubkeyTuple,
                                exit_endpoint: exitEndpointC,
                                wallet_signing_seed: signingSeedTuple,
                                multi_hop_relay: relayPtr,
                                daita_spec: daitaPtr,
                                nat_pmp_enabled: natPmpFlag,
                                bypass_cidrs: bypassBase,
                                bypass_cidrs_count: bypassCount
                            )
                            return warren_tunnel_start(&parameters, -1)
                        }
                    }
                }
            }

        guard let raw = handlePtr else { throw WarrenQuinnAdapterError.ffiStartFailed }
        return OpaquePointer(raw)
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

