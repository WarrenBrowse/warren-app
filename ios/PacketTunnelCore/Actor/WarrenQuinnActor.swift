//
//  WarrenQuinnActor.swift
//  PacketTunnelCore
//
//  Created by Warren on 2026-05-21 (C.4.3).
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  `PacketTunnelActorProtocol`-conforming actor for the Warren Quinn
//  tunnel path. Bridges Mullvad's actor-state model (used by
//  `PacketTunnelProvider` for orchestration : sleep/wake, network path
//  observation, key rotation, blocked-state error mapping) to the
//  warren-tunnel Quinn stack via `WarrenRustRuntime.WarrenQuinnAdapter`
//  (cf. `ios/WarrenRustRuntime/WarrenQuinnAdapter.swift`).
//
//  At this C.4.3 scaffold stage most methods are no-ops mirroring the
//  `GotaTunActor` pattern : the Quinn handshake + pump are driven
//  Rust-side via `warren_tunnel_ffi` (C.4.0/C.4.1). Future passes wire
//  the actor's observed state stream to the Rust-side event callback
//  (`warren_tunnel_set_event_callback`) so the parent
//  `PacketTunnelProvider` reacts to Connected / Disconnected / Failover
//  transitions.
//

import Foundation
import Network
import WarrenLogging
import WarrenTypes

/// Stub-level `PacketTunnelActorProtocol` for the Warren Quinn tunnel.
/// Real lifecycle wiring lands in C.4.3.X follow-up : connect
/// `start(options:)` to `WarrenQuinnAdapter.start(config:)`, expose
/// `observedStates` AsyncStream backed by the Rust-side event callback,
/// and forward `setErrorState(reason:)` to a blocked-state surface in
/// the parent `PacketTunnelProvider`.
public final class WarrenQuinnActor: PacketTunnelActorProtocol, @unchecked Sendable {
    private let logger = Logger(label: "WarrenQuinnActor")

    /// Snapshot of the most recently observed state. Updated by the
    /// `WarrenQuinnAdapter` event callback once C.4.3.X wires the
    /// bridge. Starts at `.disconnected` for cold launches.
    private var currentState: ObservedState = .disconnected

    public var observedState: ObservedState {
        get async { currentState }
    }

    public var observedStates: AsyncStream<ObservedState> {
        get async {
            // C.4.3.X TODO: emit values from the Rust-side event
            // callback (Connected/Disconnected/Reconnecting/Failover).
            // For now, yield a single snapshot of the current state and
            // close the stream - same UX as `GotaTunActor`.
            AsyncStream { continuation in
                continuation.yield(currentState)
                continuation.finish()
            }
        }
    }

    public init() {
        logger.info("WarrenQuinnActor initialized (C.4.3 scaffold)")
    }

    public func start(options: StartOptions) {
        // C.4.3.X TODO: marshal `options` (selected relays, source) to
        // `WarrenQuinnAdapter.WarrenTunnelConfig` and call
        // `adapter.start(config:)` ; update currentState on event.
        logger.info("start called (C.4.3 scaffold no-op)")
    }

    public func stop() {
        // C.4.3.X TODO: `adapter.stop()` + transition currentState to
        // `.disconnected` ; signal `waitUntilDisconnected` waiters.
        logger.info("stop called (C.4.3 scaffold no-op)")
    }

    public func waitUntilDisconnected() async {
        // C.4.3.X TODO: await a one-shot signal fed by the Disconnected
        // event from the Rust-side dispatcher.
        logger.info("waitUntilDisconnected called (C.4.3 scaffold no-op)")
    }

    public func onSleep() {
        // C.4.3.X TODO: hibernate the Quinn endpoint (close idle
        // connections, cancel pump tasks) to satisfy iOS background
        // suspension limits without losing the tunnel parameters.
        logger.info("onSleep called (C.4.3 scaffold no-op)")
    }

    public func onWake() {
        // C.4.3.X TODO: reconnect via `adapter.reconnect()` using the
        // last known parameters.
        logger.info("onWake called (C.4.3 scaffold no-op)")
    }

    public func updateNetworkReachability(networkPathStatus: NWPath.Status) {
        // C.4.3.X TODO: when status transitions to `.satisfied` after
        // a `.unsatisfied` window, fire `adapter.reconnect()` (Wi-Fi
        // <-> cellular handover, cf. M4.H.G HANDSHAKE 15s).
        logger.info("updateNetworkReachability called (C.4.3 scaffold no-op)")
    }

    public func reconnect(to nextRelays: NextRelays, reconnectReason: ActorReconnectReason) {
        // C.4.3.X TODO: re-marshal exit relay selection + call
        // `adapter.stop()` + `adapter.start(newConfig:)`.
        logger.info("reconnect called (C.4.3 scaffold no-op)")
    }

    public func notifyKeyRotation(date: Date?) {
        // Warren uses a static Ed25519 identity derived from the wallet
        // seed (cf. `WarrenWallet`/`warren-identity`) ; rotation is a
        // future feature (wallet recovery flow). No-op for now.
        logger.info("notifyKeyRotation called (Warren uses static wallet identity)")
    }

    public func setErrorState(reason: BlockedStateReason) {
        // C.4.3.X TODO: surface the blocked-state reason to the
        // PacketTunnelProvider so the iOS Settings panel reflects the
        // user-visible cause (revoked wallet, no signal, etc.).
        logger.info("setErrorState called (C.4.3 scaffold no-op, reason=\(reason))")
    }

    public func notifyEphemeralPeerNegotiated() {
        // Warren does NOT use the Mullvad ephemeral-peer post-quantum
        // exchange ; the Warren transport security is anchored by the
        // wallet Ed25519 identity. This method is a structural
        // requirement of `PacketTunnelActorProtocol` only.
        logger.info("notifyEphemeralPeerNegotiated called (Warren PQ-free, no-op)")
    }

    public func changeEphemeralPeerNegotiationState(
        configuration: EphemeralPeerNegotiationState,
        reconfigurationSemaphore: OneshotChannel
    ) {
        // Same rationale as `notifyEphemeralPeerNegotiated` : no-op for
        // Warren's identity model.
        logger.info("changeEphemeralPeerNegotiationState called (Warren PQ-free, no-op)")
    }
}
