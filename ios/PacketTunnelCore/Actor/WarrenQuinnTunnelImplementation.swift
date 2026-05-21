//
//  WarrenQuinnTunnelImplementation.swift
//  PacketTunnelCore
//
//  Created by Warren on 2026-05-21 (C.4.3).
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  `TunnelImplementation`-conforming class for the Warren Quinn tunnel
//  path. Slots into `PacketTunnelProvider` next to
//  `WireGuardGoTunnelImplementation` + `GotaTunTunnelImplementation` so
//  callers can pick the active backend via a debug flag without
//  modifying the parent lifecycle code. Mirrors the
//  `GotaTunTunnelImplementation` pattern : no external state observer ;
//  the `WarrenQuinnActor` handles state transitions internally,
//  ultimately driven by the Rust-side event callback registered through
//  `warren_tunnel_set_event_callback` (cf. `WarrenRustRuntime/WarrenQuinnAdapter.swift`).
//
//  At this C.4.3 scaffold stage the implementation is a structural
//  surface : `setUp/startTunnel/stopTunnel/sleep/wake` forward to the
//  no-op `WarrenQuinnActor`. C.4.3.X follow-up wires the actor to the
//  real `WarrenQuinnAdapter` instance owned by `PacketTunnelProvider`
//  (the adapter requires a `NEPacketTunnelFlow` reference + an event
//  callback, both threaded through `setUp`).
//

import WarrenLogging
import WarrenREST
@preconcurrency import NetworkExtension

/// Quinn-based tunnel implementation. Replaces
/// `WireGuardGoTunnelImplementation` for the Warren path. State
/// transitions are handled internally by `WarrenQuinnActor` ; data
/// plane flows through `WarrenRustRuntime.WarrenQuinnAdapter` →
/// `warren-tunnel::ClientTunnel` → `NEPacketTunnelFlow` (cf.
/// `.planning/c4-packet-tunnel-provider-quinn-design.md` §3-§4).
public final class WarrenQuinnTunnelImplementation: TunnelImplementation, @unchecked Sendable {
    private let logger = Logger(label: "WarrenQuinnTunnelImplementation")

    private let _actor = WarrenQuinnActor()
    public var actor: any PacketTunnelActorProtocol { _actor }

    public init() {}

    public func setUp(
        provider: NEPacketTunnelProvider,
        internalQueue: DispatchQueue,
        ipOverrideWrapper: IPOverrideWrapper,
        settingsReader: sending TunnelSettingsManager,
        apiTransportProvider: APITransportProvider
    ) {
        // C.4.3.X TODO: instantiate `WarrenQuinnAdapter` here with
        // `provider.packetFlow` + an event callback that mirrors into
        // App Group UserDefaults (cf. `WarrenAppGroupEvents` consumer
        // in TunnelViewController). Pass the adapter handle to the
        // actor so `start(options:)` can call `adapter.start(config:)`.
        logger.info("WarrenQuinnTunnelImplementation.setUp called (C.4.3 scaffold)")
    }

    public func startTunnel(options: StartOptions) async {
        // Like `GotaTunTunnelImplementation`, no external state
        // observer is started ; the actor manages transitions
        // internally. The Rust-side `warren_tunnel_set_event_callback`
        // dispatcher will feed the actor's observed state stream.
        actor.start(options: options)
    }

    public func stopTunnel() async {
        actor.stop()
        await actor.waitUntilDisconnected()
    }

    public func sleep() async {
        actor.onSleep()
    }

    public func wake() {
        actor.onWake()
    }
}
