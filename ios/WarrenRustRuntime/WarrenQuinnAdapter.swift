//
//  WarrenQuinnAdapter.swift
//  WarrenRustRuntime
//
//  Created by Warren on 2026-05-21.
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  Scaffold Swift wrapper for `warren_tunnel_ffi` (Rust crate `warren-ios`,
//  see `warren-ios/src/warren_tunnel_ffi.rs`). Implements the contract
//  documented in `.planning/c4-packet-tunnel-provider-quinn-design.md` §2.2.
//  NOT yet wired to actual FFI exports — those land in the C.4
//  implementation brief (C.4.2). This file defines the consumer-facing
//  Swift API surface that `PacketTunnelProvider` will target after the
//  WireGuardAdapter is removed.
//

import Foundation
@preconcurrency import NetworkExtension

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
    public let pubkey: Data       // 32 bytes Ed25519
    public let endpoint: String   // "IP:port"
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
    public let padding: UInt32         // padding budget in packets/sec

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
    /// The adapter was started without a `start` call yet.
    case notStarted
    /// Raw FFI failure with a Rust-side message.
    case ffi(String)
}

/// High-level Swift facade over the warren-tunnel / warren-ios FFI.
///
/// Replaces the upstream Mullvad `WgAdapter` (WireGuardKit wrapper) with
/// a Quinn-based implementation backed by `warren-tunnel`. Bridges
/// `NEPacketTunnelFlow.readPackets`/`writePackets` to a Rust-managed
/// packet queue (cf. `.planning/c4-packet-tunnel-provider-quinn-design.md`
/// §3.2 for the iOS-specific utun fd handling discussion).
public actor WarrenQuinnAdapter {
    private let packetFlow: NEPacketTunnelFlow
    private let eventCallback: @Sendable (WarrenTunnelEvent) -> Void
    private var handle: OpaquePointer?

    public init(
        packetFlow: NEPacketTunnelFlow,
        eventCallback: @escaping @Sendable (WarrenTunnelEvent) -> Void
    ) {
        self.packetFlow = packetFlow
        self.eventCallback = eventCallback
    }

    /// Starts the Warren Quinn tunnel with the given configuration.
    /// On success, spawns a packet pump task bridging
    /// `packetFlow.readPackets` ↔ Rust-side queue.
    public func start(config: WarrenTunnelConfig) async throws {
        // TODO C.4.2: marshal config -> WarrenTunnelParametersC + call
        // warren_tunnel_start(parameters, packet_fd) via FFI.
        // Bridge NEPacketTunnelFlow.readPackets / writePackets to the
        // Rust-managed queue (cf. design doc §3.2).
        _ = config
        throw WarrenQuinnAdapterError.ffi("WarrenQuinnAdapter.start() not implemented (C.4.2 pending)")
    }

    /// Stops the tunnel and releases all resources. Idempotent.
    public func stop() async {
        // TODO C.4.2: call warren_tunnel_stop(handle) via FFI.
        handle = nil
    }

    /// Triggers a tunnel reconnect (e.g. on Wi-Fi <-> cellular handover).
    /// Uses the `warren-backoff::Backoff::HANDSHAKE` window (15s, cf. M4.H.G).
    public func reconnect() async {
        // TODO C.4.2: call warren_tunnel_reconnect(handle) via FFI.
    }

    /// Returns the current tunnel status without blocking.
    public func status() async -> WarrenTunnelStatus {
        // TODO C.4.2: call warren_tunnel_status(handle, &status) via FFI.
        return WarrenTunnelStatus(
            state: handle == nil ? .disconnected : .connected,
            bytesIn: 0,
            bytesOut: 0,
            connectedDurationSeconds: nil,
            failoverCount: 0
        )
    }
}
