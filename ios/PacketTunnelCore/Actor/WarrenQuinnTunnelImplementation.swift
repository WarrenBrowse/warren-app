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

import Foundation
import WarrenLogging
import WarrenREST
import WarrenRustRuntime
@preconcurrency import NetworkExtension

/// Quinn-based tunnel implementation. Replaces
/// `WireGuardGoTunnelImplementation` for the Warren path. State
/// transitions are handled internally by `WarrenQuinnActor` ; data
/// plane flows through `WarrenRustRuntime.WarrenQuinnAdapter` →
/// `warren-tunnel::ClientTunnel` → `NEPacketTunnelFlow` (cf.
/// `.planning/c4-packet-tunnel-provider-quinn-design.md` §3-§4).
public final class WarrenQuinnTunnelImplementation: TunnelImplementation, @unchecked Sendable {
    private let logger = Logger(label: "WarrenQuinnTunnelImplementation")

    private let _actor: WarrenQuinnActor
    public var actor: any PacketTunnelActorProtocol { _actor }

    /// The Rust-backed Quinn adapter. Created lazily in
    /// [`setUp(provider:...)`] because it needs the live
    /// `NEPacketTunnelFlow` from the system-provided
    /// `NEPacketTunnelProvider`. Nil until `setUp` has been called.
    private var adapter: WarrenQuinnAdapter?

    /// Strong reference to the App Group `UserDefaults` used to
    /// broadcast tunnel events to the main app. Pinned for the lifetime
    /// of the implementation so the event callback can fire-and-forget
    /// without re-resolving the suite.
    private let appGroupDefaults: UserDefaults?

    /// Background Task that periodically snapshots the adapter status
    /// + writes counters into App Group `UserDefaults` so the
    /// main-app `WarrenTunnelStatisticsView` can render live numbers.
    /// Spawned in `setUp(provider:...)` ; cancelled in deinit.
    private var statsBroadcastTask: Task<Void, Never>?

    public init() {
        self._actor = WarrenQuinnActor()
        // Best-effort App Group defaults handle. When the bundle is
        // not yet configured (unit tests), this returns nil and the
        // event broadcast becomes a no-op.
        let suite = Bundle.main.object(forInfoDictionaryKey: "ApplicationSecurityGroupIdentifier") as? String
        self.appGroupDefaults = suite.flatMap { UserDefaults(suiteName: $0) }
    }

    public func setUp(
        provider: NEPacketTunnelProvider,
        internalQueue: DispatchQueue,
        ipOverrideWrapper: IPOverrideWrapper,
        settingsReader: sending TunnelSettingsManager,
        apiTransportProvider: APITransportProvider
    ) {
        logger.info("WarrenQuinnTunnelImplementation.setUp (instantiate adapter + wire event callback)")
        // Adapter owns the NEPacketTunnelFlow bridge to the Rust IosTun
        // PacketDevice. The event callback mirrors transitions + NAT-PMP
        // events into the App Group so the main app's
        // `WarrenAppGroupEvents` observer surfaces them in UI
        // (FailoverBanner + ObfuscationIndicator + NAT-PMP port row).
        let appDefaults = appGroupDefaults
        let weakActor = _actor
        let adapter = WarrenQuinnAdapter(
            packetFlow: provider.packetFlow,
            eventCallback: { event in
                // Surface to App Group for cross-process UI consumers.
                Self.broadcastEvent(event, into: appDefaults)
                // Feed actor's observed state stream (C.4.3.X follow-up
                // will turn this into a proper AsyncStream backed by the
                // events ; for now we update the snapshot directly).
                weakActor.applyEvent(event)
            }
        )
        self.adapter = adapter
        _actor.bindAdapter(adapter)

        // Push the wallet signing seed into the actor so
        // `start(options:)` can marshal it into the FFI config. The
        // Keychain entry uses `kSecAttrAccessibleWhenUnlockedThisDeviceOnly`
        // and lives in an App Group accessible from both the main app
        // and this PacketTunnel extension. C.4.3.Z TODO : extract the
        // Keychain read helper to `Shared/` so PacketTunnelCore can
        // call it directly (currently in WarrenVPN target only) ; for
        // now log the deferred state so the gap is observable.
        logger.info(
            "WarrenQuinnTunnelImplementation.setUp : adapter bound, wallet seed bridge deferred to C.4.3.Z (Shared/ Keychain helper extraction)"
        )

        startStatsBroadcastTask(adapter: adapter)
    }

    deinit {
        statsBroadcastTask?.cancel()
    }

    /// Spawn the periodic stats snapshot task that drives
    /// `WarrenTunnelStatisticsView` in the main app. Pulls
    /// `adapter.status()` every 2 s and writes counters to App Group
    /// `UserDefaults` under the `WarrenAppGroupKey.*` keys. 2 s strikes
    /// a balance between freshness in the UI and minimal CPU spend in
    /// the extension's tight background time budget.
    private func startStatsBroadcastTask(adapter: WarrenQuinnAdapter) {
        statsBroadcastTask?.cancel()
        let defaults = appGroupDefaults
        statsBroadcastTask = Task.detached(priority: .background) {
            while !Task.isCancelled {
                let snapshot = adapter.status()
                Self.broadcastStats(snapshot, into: defaults)
                try? await Task.sleep(nanoseconds: 2_000_000_000)
            }
        }
    }

    private static func broadcastStats(_ status: WarrenTunnelStatus, into defaults: UserDefaults?) {
        guard let defaults else { return }
        defaults.set(Int(status.bytesIn), forKey: WarrenAppGroupKey.bytesIn.rawValue)
        defaults.set(Int(status.bytesOut), forKey: WarrenAppGroupKey.bytesOut.rawValue)
        defaults.set(
            Int(status.connectedDurationSeconds ?? 0),
            forKey: WarrenAppGroupKey.connectedDurationSeconds.rawValue
        )
        defaults.set(Int(status.failoverCount), forKey: WarrenAppGroupKey.failoverCount.rawValue)
        defaults.set(Self.stateLabel(for: status.state), forKey: WarrenAppGroupKey.stateLabel.rawValue)
    }

    /// Localized label for the current state. Falls back to "Failed"
    /// with the underlying message for the `.failed(_)` variant.
    private static func stateLabel(for state: WarrenTunnelStatus.State) -> String {
        switch state {
        case .disconnected: return "Disconnected"
        case .connecting: return "Connecting"
        case .connected: return "Connected"
        case .reconnecting: return "Reconnecting"
        case .failed(let reason): return "Failed (\(reason))"
        }
    }

    public func startTunnel(options: StartOptions) async {
        // Forward to the actor ; the actor decides whether to call
        // `adapter.start(config:)` based on the resolved tunnel config
        // (built from `options` + tunnel settings). No external state
        // observer like WireGuardGo : the Rust-side event callback
        // drives state transitions through `applyEvent`.
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

    // MARK: - App Group event broadcasting

    /// Mirror a `WarrenTunnelEvent` into the App Group `UserDefaults`
    /// keys consumed by `WarrenAppGroupEvents` in the main-app process.
    /// Keys mirror those declared in
    /// `ios/WarrenVPN/View controllers/Tunnel/WarrenAppGroupEvents.swift`.
    private static func broadcastEvent(_ event: WarrenTunnelEvent, into defaults: UserDefaults?) {
        guard let defaults else { return }
        switch event {
        case .failover(let exit):
            defaults.set(exit, forKey: WarrenAppGroupKey.lastFailoverExit.rawValue)
            defaults.set(Date(), forKey: WarrenAppGroupKey.lastFailoverAt.rawValue)
        case .natPmpMapped(_, let externalPort, _),
             .natPmpRenewed(let externalPort):
            defaults.set(Int(externalPort), forKey: WarrenAppGroupKey.natPmpExternalPort.rawValue)
        case .connected, .disconnected, .reconnecting, .natPmpFailed:
            // Transient transitions surface via the actor's
            // observedStates AsyncStream, not via App Group keys.
            break
        }
    }
}
