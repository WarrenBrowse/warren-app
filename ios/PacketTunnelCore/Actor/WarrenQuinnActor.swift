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
import WarrenSettings
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

    /// 32-byte Ed25519 signing seed derived from the user wallet,
    /// loaded via the cross-process Keychain bridge by the owning
    /// `WarrenQuinnTunnelImplementation` and pushed in via
    /// [`bindWalletSigningSeed(_:)`]. `nil` until the bridge fires ;
    /// `start(options:)` falls back to a logged no-op until then.
    /// Cleared by [`stop()`].
    private var walletSigningSeed: Data?

    public var observedState: ObservedState {
        get async { snapshotState() }
    }

    /// Non-async lock-guarded snapshot used by the async accessor.
    /// Swift 6 strict concurrency forbids `NSLock.lock()` from inside
    /// an async context (it's not Sendable / async-safe), so we hop
    /// through a regular sync function that uses `withLock`.
    private func snapshotState() -> ObservedState {
        stateLock.withLock { currentState }
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

    /// Push the wallet Ed25519 signing seed in from the main app
    /// (Keychain bridge crosses the WarrenVPN → PacketTunnelCore
    /// target boundary). Called by `WarrenQuinnTunnelImplementation`
    /// after `bindAdapter(_:)` once the wallet has been resolved.
    /// 32-byte seed is held by value ; the actor's `stop()` clears it
    /// to keep the memory window narrow.
    public func bindWalletSigningSeed(_ seed: Data) {
        stateLock.lock()
        self.walletSigningSeed = seed
        stateLock.unlock()
    }

    /// Apply a `WarrenTunnelEvent` produced by the Rust-side event
    /// callback. Maps the typed event into an `ObservedState` and
    /// pushes it onto the `observedStates` stream + the snapshot.
    /// Called from the Rust dispatcher thread ; thread-safe via
    /// `stateLock`.
    public func applyEvent(_ event: WarrenTunnelEvent) {
        // C.4.3.Z TODO : `ObservedState.connected/.reconnecting` require
        // an `ObservedConnectionState` payload (selectedRelay + IP +
        // etc.) - Warren's actor doesn't have that context yet. For
        // the scaffold we map every non-disconnect to `.initial` and
        // only fire `.disconnected` definitively. Real wiring lands
        // once `WarrenQuinnTunnelImplementation.startStatsBroadcastTask`
        // pushes the connection details up to the actor.
        let nextState: ObservedState
        var disconnectContinuation: CheckedContinuation<Void, Never>? = nil
        switch event {
        case .connected, .reconnecting, .failover:
            nextState = .initial
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
        stateLock.lock()
        let adapter = self.adapter
        let seed = self.walletSigningSeed
        stateLock.unlock()

        guard let adapter else {
            logger.warning("start called before bindAdapter ; ignoring")
            return
        }
        guard let seed, seed.count == 32 else {
            logger.warning("start called before bindWalletSigningSeed ; cannot derive identity")
            return
        }
        guard let selectedRelays = options.selectedRelays else {
            logger.warning("start called without selectedRelays ; relay selection deferred to UI layer")
            return
        }

        // Build the WarrenTunnelConfig from the selected exit relay's
        // resolved socket address + 32-byte pubkey. Multi-hop entry
        // relay is honored when present (Session R B.1.8 closed).
        let exit = selectedRelays.exit
        let exitEndpointStr = "\(exit.endpoint.socketAddress)"
        let exitPubkey = exit.endpoint.publicKey

        let multiHop: WarrenRelayConfig?
        if let entry = selectedRelays.entry {
            multiHop = WarrenRelayConfig(
                pubkey: entry.endpoint.publicKey,
                endpoint: "\(entry.endpoint.socketAddress)",
                countryCode: entry.location.countryCode
            )
        } else {
            multiHop = nil
        }

        // DAITA: read the persisted setting (written by SettingsDAITAView via
        // DAITATunnelSettingsViewModel). The client only signals the request;
        // the exit picks the Maybenot machine and returns it in SetupAck, so
        // the spec content here is a placeholder - only its presence matters
        // (the warren-ios FFI treats a non-null daita_spec as "DAITA on").
        let daitaEnabled = (try? SettingsManager.readSettings().daita.isEnabled) ?? false
        let daitaSpec: WarrenDaitaSpec? =
            daitaEnabled ? WarrenDaitaSpec(machineSeedHex: "", padding: 0) : nil

        let config = WarrenTunnelConfig(
            exitPubkey: exitPubkey,
            exitEndpoint: exitEndpointStr,
            walletSigningKey: seed,
            multiHopRelay: multiHop,
            daitaSpec: daitaSpec,
            natPmpEnabled: false,  // settings-driven, deferred (needs V9 schema)
            bypassCidrs: []  // settings-driven, deferred
        )

        do {
            try adapter.start(config: config)
            logger.info("Warren tunnel started via WarrenQuinnAdapter")
        } catch {
            logger.error("WarrenQuinnAdapter.start failed: \(error)")
            applyEvent(.disconnected)
        }
    }

    public func stop() {
        stateLock.lock()
        let adapter = self.adapter
        // Clear the wallet seed so its memory window is narrow.
        // bindWalletSigningSeed will re-push on next start.
        self.walletSigningSeed = nil
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
        // Swift 6 strict concurrency forbids NSLock.lock from async
        // context. Hop through sync helpers that take/release the
        // lock as a single atomic operation.
        if isAlreadyDisconnected() {
            return
        }
        await withCheckedContinuation { (continuation: CheckedContinuation<Void, Never>) in
            installDisconnectedSignal(continuation)
        }
    }

    private func isAlreadyDisconnected() -> Bool {
        stateLock.withLock {
            if case .disconnected = currentState { return true }
            return false
        }
    }

    private func installDisconnectedSignal(_ continuation: CheckedContinuation<Void, Never>) {
        stateLock.withLock {
            disconnectedSignal = continuation
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
