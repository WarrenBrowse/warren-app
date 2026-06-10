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
//  The Quinn handshake + pump run Rust-side via `warren_tunnel_ffi`;
//  the actor's observed-state stream is fed from the Rust-side event
//  callback (`warren_tunnel_set_event_callback`) so the parent
//  `PacketTunnelProvider` reacts to Connected / Disconnected / Failover
//  transitions.
//

import Foundation
import Network
import WarrenLogging
import WarrenREST
import WarrenRustRuntime
import WarrenSettings
import WarrenTypes

/// `PacketTunnelActorProtocol` for the Warren Quinn tunnel: `start(options:)`
/// drives `WarrenQuinnAdapter.start(config:)`, `observedStates` is an
/// AsyncStream backed by the Rust-side event callback, and
/// `setErrorState(reason:)` surfaces a blocked state to the parent
/// `PacketTunnelProvider`.
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
    private var adapter: WarrenQuinnAdapting?

    /// 32-byte Ed25519 signing seed derived from the user wallet,
    /// loaded via the cross-process Keychain bridge by the owning
    /// `WarrenQuinnTunnelImplementation` and pushed in via
    /// [`bindWalletSigningSeed(_:)`]. `nil` until the bridge fires ;
    /// `start(options:)` falls back to a logged no-op until then.
    /// Cleared by [`stop()`].
    private var walletSigningSeed: Data?

    /// Connection details captured at [`start(options:)`] so the Rust-side
    /// `.connected`/`.reconnecting`/`.failover` events (which carry no payload)
    /// can be surfaced as a real `ObservedConnectionState` (relay + IP + port)
    /// the UI can render. `nil` before the first start ; events that arrive
    /// without it fall back to `.initial`. Cleared by [`stop()`].
    private struct ConnectionContext {
        let selectedRelays: SelectedRelays
        let relayConstraints: RelayConstraints
        let remotePort: UInt16
        let isDaitaEnabled: Bool
    }
    private var connectionContext: ConnectionContext?

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
    public func bindAdapter(_ adapter: WarrenQuinnAdapting) {
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
        // NAT-PMP events are state-neutral (surfaced via the App Group
        // broadcast layer), so they never touch the observed state.
        switch event {
        case .natPmpMapped, .natPmpRenewed, .natPmpFailed:
            return
        default:
            break
        }

        var disconnectContinuation: CheckedContinuation<Void, Never>? = nil
        stateLock.lock()
        // `connected`/`reconnecting`/`failover` carry no payload, so we rebuild
        // the connection state from the context captured at `start()`. Without
        // that context (an event before start), there is nothing to show, so
        // we stay `.initial` rather than fabricate a relay.
        let nextState: ObservedState
        switch event {
        case .connected:
            nextState = connectionContext
                .map { .connected(observedConnectionState(from: $0)) } ?? .initial
        case .reconnecting, .failover:
            nextState = connectionContext
                .map { .reconnecting(observedConnectionState(from: $0)) } ?? .initial
        case .disconnected:
            nextState = .disconnected
        case .natPmpMapped, .natPmpRenewed, .natPmpFailed:
            stateLock.unlock()
            return
        }
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

    /// Build the UI-facing connection state from the captured context.
    /// Pure (takes no lock) ; callers hold `stateLock`. Warren is QUIC/UDP
    /// and post-quantum-free, hence the fixed `transportLayer`/`isPostQuantum`.
    private func observedConnectionState(from ctx: ConnectionContext) -> ObservedConnectionState {
        ObservedConnectionState(
            selectedRelays: ctx.selectedRelays,
            relayConstraints: ctx.relayConstraints,
            networkReachability: .reachable,
            connectionAttemptCount: 0,
            transportLayer: .udp,
            remotePort: ctx.remotePort,
            isPostQuantum: false,
            isDaitaEnabled: ctx.isDaitaEnabled
        )
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

        let (config, context) = makeConfigAndContext(selectedRelays: selectedRelays, seed: seed)
        stateLock.lock()
        connectionContext = context
        stateLock.unlock()

        do {
            try adapter.start(config: config)
            logger.info("Warren tunnel started via WarrenQuinnAdapter")
        } catch {
            logger.error("WarrenQuinnAdapter.start failed: \(error)")
            applyEvent(.disconnected)
        }
    }

    /// Build the FFI tunnel config + the UI connection context from a relay
    /// selection. Shared by `start(options:)` and the relay-change
    /// `reconnect(to:)` path so both marshal identically. DAITA presence is
    /// settings-driven (the exit picks the Maybenot machine, so the spec
    /// content is a placeholder). Multi-hop is forced off (see below).
    private func makeConfigAndContext(
        selectedRelays: SelectedRelays,
        seed: Data
    ) -> (WarrenTunnelConfig, ConnectionContext) {
        let exit = selectedRelays.exit
        // The Warren Quinn FFI does not consume a multi-hop entry relay yet:
        // the iOS relay model carries none of the signed relay/exit descriptors
        // the multi-hop handshake needs. Force single-hop here so a user whose
        // persisted setting is multi-hop is never silently single-hopped at the
        // data plane while believing otherwise. The multi-hop settings UI is
        // gated to match. Re-enable by building WarrenRelayConfig from
        // selectedRelays.entry once the FFI and the relay descriptors land.
        let multiHop: WarrenRelayConfig? = nil
        let daitaEnabled = (try? SettingsManager.readSettings().daita.isEnabled) ?? false
        let daitaSpec: WarrenDaitaSpec? =
            daitaEnabled ? WarrenDaitaSpec(machineSeedHex: "", padding: 0) : nil

        let config = WarrenTunnelConfig(
            exitPubkey: exit.endpoint.publicKey,
            exitEndpoint: "\(exit.endpoint.socketAddress)",
            walletSigningKey: seed,
            multiHopRelay: multiHop,
            daitaSpec: daitaSpec,
            natPmpEnabled: false,  // settings-driven, deferred (needs V9 schema)
            bypassCidrs: []  // settings-driven, deferred
        )
        // Settings reads are best-effort: a unit test or a not-yet-provisioned
        // store falls back to defaults.
        let relayConstraints =
            (try? SettingsManager.readSettings().relayConstraints) ?? RelayConstraints()
        let context = ConnectionContext(
            selectedRelays: selectedRelays,
            relayConstraints: relayConstraints,
            remotePort: exit.endpoint.socketAddress.port,
            isDaitaEnabled: daitaEnabled
        )
        return (config, context)
    }

    public func stop() {
        stateLock.lock()
        let adapter = self.adapter
        // Clear the wallet seed so its memory window is narrow.
        // bindWalletSigningSeed will re-push on next start.
        self.walletSigningSeed = nil
        self.connectionContext = nil
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
        switch nextRelays {
        case let .preSelected(selectedRelays):
            // A relay change: the app re-selected (Warren has no in-actor
            // selector), so rebuild the config for the new exit and restart
            // the adapter.
            restartAdapter(with: selectedRelays)
        case .current, .random:
            // No new relays to marshal and no in-actor selector, so the best
            // the actor can do is re-handshake the current exit. True relay
            // changes arrive as `.preSelected` from the app.
            stateLock.lock()
            let adapter = self.adapter
            stateLock.unlock()
            adapter?.reconnect()
        }
    }

    /// Tear down the current Quinn session and bring up a new one against
    /// `selectedRelays` (relay-change path). Holds no lock across the adapter
    /// calls.
    private func restartAdapter(with selectedRelays: SelectedRelays) {
        stateLock.lock()
        let adapter = self.adapter
        let seed = self.walletSigningSeed
        stateLock.unlock()

        guard let adapter, let seed, seed.count == 32 else {
            logger.warning("reconnect(to:) before a started session ; ignoring")
            return
        }

        let (config, context) = makeConfigAndContext(selectedRelays: selectedRelays, seed: seed)
        stateLock.lock()
        connectionContext = context
        stateLock.unlock()

        adapter.stop()
        do {
            try adapter.start(config: config)
            logger.info("Warren tunnel reconnected to a new exit via WarrenQuinnAdapter")
        } catch {
            logger.error("WarrenQuinnAdapter.start (relay change) failed: \(error)")
            applyEvent(.disconnected)
        }
    }

    public func notifyKeyRotation(date: Date?) {
        // Warren uses a static Ed25519 identity derived from the wallet
        // seed (cf. `WarrenWallet`/`warren-identity`) ; rotation is a
        // future feature (wallet recovery flow). No-op for now.
        logger.info("notifyKeyRotation called (Warren uses static wallet identity)")
    }

    public func setErrorState(reason: BlockedStateReason) {
        // Surface the blocked-state reason as an `.error` observed state so
        // the parent `PacketTunnelProvider` chain (BlockedStateErrorMapper ->
        // IPC -> TunnelState.error -> UI) reflects the user-visible cause
        // (device revoked/logged out, no signal, etc.). Without this a
        // failed device check (PacketTunnelProvider.startDeviceCheckInner)
        // would vanish silently.
        logger.info("setErrorState: entering blocked state (reason=\(reason))")
        let nextState: ObservedState = .error(ObservedBlockedState(reason: reason))
        stateLock.lock()
        currentState = nextState
        let continuation = observedStatesContinuation
        stateLock.unlock()
        continuation?.yield(nextState)
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
