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
import WarrenRustRuntime
import WarrenTypes

/// Stub-level `PacketTunnelActorProtocol` for the Warren Quinn tunnel.
/// Real lifecycle wiring lands in C.4.3.X follow-up : connect
/// `start(options:)` to `WarrenQuinnAdapter.start(config:)`, expose
/// `observedStates` AsyncStream backed by the Rust-side event callback,
/// and forward `setErrorState(reason:)` to a blocked-state surface in
/// the parent `PacketTunnelProvider`.
public final class WarrenQuinnActor: PacketTunnelActorProtocol, @unchecked Sendable {
    private let logger = Logger(label: "WarrenQuinnActor")

    /// Snapshot of the most recently observed state. Updated by
    /// [`applyEvent(_:)`] when the Rust-side event callback fires.
    private let stateLock = NSLock()
    private var currentState: ObservedState = .disconnected

    /// Continuation feeding the [`observedStates`] AsyncStream. Set
    /// when a consumer first awaits `observedStates`. Subsequent
    /// `applyEvent(_:)` calls push transitions onto it.
    private var observedStatesContinuation: AsyncStream<ObservedState>.Continuation?

    /// One-shot signal fired when the tunnel transitions to
    /// `.disconnected`. `waitUntilDisconnected()` awaits it.
    private var disconnectedSignal: CheckedContinuation<Void, Never>?

    /// Strong reference to the [`WarrenQuinnAdapter`] bound by the
    /// owning `WarrenQuinnTunnelImplementation` via [`bindAdapter(_:)`].
    /// `nil` until `bindAdapter` is called ; `start(options:)` is a
    /// no-op until then.
    private var adapter: WarrenQuinnAdapter?

    public var observedState: ObservedState {
        get async {
            stateLock.lock()
            defer { stateLock.unlock() }
            return currentState
        }
    }

    public var observedStates: AsyncStream<ObservedState> {
        get async {
            AsyncStream { continuation in
                stateLock.lock()
                self.observedStatesContinuation = continuation
                let snapshot = self.currentState
                stateLock.unlock()
                continuation.yield(snapshot)
                continuation.onTermination = { @Sendable [weak self] _ in
                    self?.stateLock.lock()
                    self?.observedStatesContinuation = nil
                    self?.stateLock.unlock()
                }
            }
        }
    }

    public init() {
        logger.info("WarrenQuinnActor initialized (C.4.3.X wired)")
    }

    // MARK: - Binding + event dispatch (C.4.3.X)

    /// Bind a `WarrenQuinnAdapter` instance owned by the parent
    /// `WarrenQuinnTunnelImplementation`. Called once during the
    /// implementation's `setUp(provider:...)` ; subsequent calls are
    /// idempotent (replace).
    public func bindAdapter(_ adapter: WarrenQuinnAdapter) {
        stateLock.lock()
        self.adapter = adapter
        stateLock.unlock()
    }

    /// Apply a `WarrenTunnelEvent` produced by the Rust-side event
    /// callback. Maps the typed event into an `ObservedState` and
    /// pushes it onto the `observedStates` stream + the snapshot.
    /// Called from the Rust dispatcher thread ; thread-safe via
    /// `stateLock`.
    public func applyEvent(_ event: WarrenTunnelEvent) {
        let nextState: ObservedState
        var disconnectContinuation: CheckedContinuation<Void, Never>? = nil
        switch event {
        case .connected:
            nextState = .connected
        case .reconnecting, .failover:
            nextState = .reconnecting
        case .disconnected:
            nextState = .disconnected
        case .natPmpMapped, .natPmpRenewed, .natPmpFailed:
            // NAT-PMP events do not change the tunnel state ; they are
            // surfaced separately via the App Group broadcast layer.
            return
        }

        stateLock.lock()
        currentState = nextState
        let continuation = observedStatesContinuation
        if case .disconnected = nextState {
            disconnectContinuation = disconnectedSignal
            disconnectedSignal = nil
        }
        stateLock.unlock()

        continuation?.yield(nextState)
        disconnectContinuation?.resume()
    }

    public func start(options: StartOptions) {
        // C.4.3.Y TODO: marshal `options` (selected relays, source) +
        // the persisted wallet identity (via WarrenWallet/Keychain) +
        // a freshly-selected relay descriptor into
        // `WarrenTunnelConfig` and call `adapter.start(config:)`.
        // Today the adapter is bound but the config marshalling needs
        // a settings reader handoff to thread the exit/relay/wallet
        // through ; tracked separately.
        logger.info("start called (adapter bound, config marshalling deferred to C.4.3.Y)")
    }

    public func stop() {
        stateLock.lock()
        let adapter = self.adapter
        stateLock.unlock()
        adapter?.stop()
        // The adapter's `stop()` triggers a Disconnected event on the
        // Rust side ; `applyEvent(.disconnected)` will fire and resume
        // any waiter on `waitUntilDisconnected()`. As a fallback (in
        // case the Rust dispatcher never fires post-stop), we also
        // synthesize the transition here.
        applyEvent(.disconnected)
    }

    public func waitUntilDisconnected() async {
        stateLock.lock()
        if case .disconnected = currentState {
            stateLock.unlock()
            return
        }
        await withCheckedContinuation { (continuation: CheckedContinuation<Void, Never>) in
            disconnectedSignal = continuation
            stateLock.unlock()
        }
    }

    public func onSleep() {
        // iOS background suspension : pause the inbound pump but keep
        // the Quinn connection alive so `onWake` can resume instantly
        // without paying the handshake cost. Falls back to `stop()` on
        // longer suspensions when the connection is torn down by the
        // peer's idle timeout.
        stateLock.lock()
        let adapter = self.adapter
        stateLock.unlock()
        adapter?.pause()
    }

    public func onWake() {
        // Resume the paused pump. If the Quinn connection was torn
        // down during suspension (peer idle timeout), the
        // adapter-internal reconnect logic will fire on the next
        // packet attempt.
        stateLock.lock()
        let adapter = self.adapter
        stateLock.unlock()
        adapter?.resume()
    }

    private var lastNetworkPathStatus: NWPath.Status = .satisfied
    public func updateNetworkReachability(networkPathStatus: NWPath.Status) {
        // Fire reconnect on Wi-Fi <-> cellular handover (transition
        // from `.unsatisfied` to `.satisfied`).
        let shouldReconnect: Bool = {
            switch (lastNetworkPathStatus, networkPathStatus) {
            case (.unsatisfied, .satisfied):
                return true
            default:
                return false
            }
        }()
        lastNetworkPathStatus = networkPathStatus
        guard shouldReconnect else { return }
        stateLock.lock()
        let adapter = self.adapter
        stateLock.unlock()
        adapter?.reconnect()
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
