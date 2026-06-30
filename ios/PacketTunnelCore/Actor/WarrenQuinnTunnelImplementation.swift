//
//  WarrenQuinnTunnelImplementation.swift
//  PacketTunnelCore
//
//  Created by Warren on 2026-05-21.
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
//  `setUp/startTunnel/stopTunnel/sleep/wake` forward to the
//  `WarrenQuinnActor`, which owns the `WarrenQuinnAdapter` instance (the
//  adapter requires a `NEPacketTunnelFlow` reference + an event callback,
//  both threaded through `setUp`).
//

import Foundation
import WarrenLogging
import WarrenREST
import WarrenRustRuntime
@preconcurrency import NetworkExtension

/// Re-applies the tunnel network settings with an exit-allocated IPv4.
/// `PacketTunnelProvider` (the concrete `NEPacketTunnelProvider`) conforms
/// to it; the Warren Quinn implementation calls it when the multi-hop
/// circuit reports a fresh `IpAssign`, so the TUN source IP matches what
/// the exit expects (else return traffic is dropped).
/// Defined here so `PacketTunnelCore` can drive the reassign without
/// depending on the `PacketTunnel` target's concrete provider type.
public protocol WarrenTunnelIPReassigning: AnyObject, Sendable {
    /// Apply `address`/`prefixLength` as the tunnel's IPv4 interface
    /// address, keeping all other settings (DNS, MTU, IPv6 blackhole,
    /// default route) intact.
    func reapplyWarrenTunnelIPv4(address: String, prefixLength: Int) async
}

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

    /// App Group suite name (Sendable String, cf. Swift 6 strict
    /// concurrency). Used to re-resolve `UserDefaults` inside detached
    /// tasks (UserDefaults is not Sendable even though it's
    /// thread-safe at runtime).
    private let appGroupSuiteName: String?

    /// Cached `UserDefaults` for the synchronous broadcastEvent path.
    /// Re-resolved per-call in the async statsBroadcastTask to avoid
    /// the Sendable capture.
    private let appGroupDefaults: UserDefaults?

    /// Background Task that periodically snapshots the adapter status
    /// + writes counters into App Group `UserDefaults` so the
    /// main-app `WarrenTunnelStatisticsView` can render live numbers.
    /// Spawned in `setUp(provider:...)` ; cancelled in deinit.
    private var statsBroadcastTask: Task<Void, Never>?

    /// Background Task that periodically re-fetches the signed multi-hop
    /// directory and re-selects a fresh circuit when the fleet's
    /// `generation` advances (the desktop daemon refreshes on a timer;
    /// iOS otherwise fetches once per connect). Cancelled in `stopTunnel`
    /// and deinit.
    private var directoryRefreshTask: Task<Void, Never>?

    /// The trusted directory `generation` the current session was started
    /// with. A periodic refresh that observes a strictly higher generation
    /// triggers a re-selection. `-1` until the first fetch.
    private var lastDirectoryGeneration: Int64 = -1

    /// The relays the session was started with, reused to drive a
    /// directory-refresh reconnect via `reconnect(to: .preSelected(...))`.
    private var warrenSelectedRelays: SelectedRelays?

    public init() {
        self._actor = WarrenQuinnActor()
        // Best-effort App Group defaults handle. When the bundle is
        // not yet configured (unit tests), this returns nil and the
        // event broadcast becomes a no-op.
        let suite = Bundle.main.object(forInfoDictionaryKey: "ApplicationSecurityGroupIdentifier") as? String
        self.appGroupSuiteName = suite
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
        // UserDefaults isn't Sendable in Swift 6 strict concurrency.
        // Capture the suite name (Sendable String) and re-resolve the
        // UserDefaults inside the @Sendable event callback.
        let suiteName = appGroupSuiteName
        let weakActor = _actor
        // The concrete provider re-applies the tunnel settings when the
        // exit allocates an IPv4. Single-hop dev providers that do not
        // conform simply skip the reassign (closure stays nil).
        let ipAssignCallback: (@Sendable (WarrenTunnelIpAssign) -> Void)?
        if let reassigner = provider as? WarrenTunnelIPReassigning {
            // The Rust reassign task fires sequentially and dedups by the last
            // forwarded address, and the exit's allocation is pubkey-sticky
            // (the IP does not change across reconnects), so the unstructured
            // `Task` hop here cannot reorder two *different* addresses in
            // practice. setTunnelNetworkSettings is itself serialized by the OS.
            ipAssignCallback = { [weak reassigner] assign in
                Task {
                    await reassigner?.reapplyWarrenTunnelIPv4(
                        address: assign.ipv4,
                        prefixLength: assign.prefixLength
                    )
                }
            }
        } else {
            ipAssignCallback = nil
        }
        let adapter = WarrenQuinnAdapter(
            packetFlow: provider.packetFlow,
            eventCallback: { [weak self] event in
                let defaults = suiteName.flatMap { UserDefaults(suiteName: $0) }
                Self.broadcastEvent(event, into: defaults)
                // A TOFU pin mismatch fails the connection closed, so it
                // arrives as a `.disconnected` event. Drain the recorded
                // mismatch (if any) and broadcast it so the main app can
                // present the Trust / Report / Reject alert.
                if case .disconnected = event,
                   let mismatch = self?.adapter?.takePinMismatch() {
                    Self.broadcastPinMismatch(mismatch, into: defaults)
                }
                weakActor.applyEvent(event)
            },
            ipAssignCallback: ipAssignCallback
        )
        self.adapter = adapter
        _actor.bindAdapter(adapter)

        // Push the wallet Ed25519 signing seed into the actor. The
        // Keychain entry lives in `Shared/` so the PacketTunnel extension
        // can read it via the same
        // `kSecAttrAccessibleWhenUnlockedThisDeviceOnly` attribute.
        // Failure here means the wallet was never provisioned: the actor's
        // `start(options:)` logs + bails out cleanly.
        do {
            let mnemonic = try WarrenWalletKeychain.load()
            let wallet = try WarrenWallet.fromMnemonic(mnemonic)
            _actor.bindWalletSigningSeed(wallet.seed)
            wallet.forgetSecret()
            logger.info("Wallet seed bound to WarrenQuinnActor")
        } catch {
            logger.warning(
                "Wallet seed bind failed: \(error). start(options:) will be a no-op until OnboardingWizard provisions a wallet."
            )
        }

        startStatsBroadcastTask(adapter: adapter)
    }

    deinit {
        statsBroadcastTask?.cancel()
        directoryRefreshTask?.cancel()
    }

    /// Spawn the periodic stats snapshot task that drives
    /// `WarrenTunnelStatisticsView` in the main app. Pulls
    /// `adapter.status()` every 2 s and writes counters to App Group
    /// `UserDefaults` under the `WarrenAppGroupKey.*` keys. 2 s strikes
    /// a balance between freshness in the UI and minimal CPU spend in
    /// the extension's tight background time budget.
    private func startStatsBroadcastTask(adapter: WarrenQuinnAdapter) {
        statsBroadcastTask?.cancel()
        // UserDefaults isn't Sendable in Swift 6 strict concurrency,
        // even though it IS thread-safe at runtime. Capture the suite
        // name as a String (Sendable) and re-resolve UserDefaults
        // inside the detached task.
        let suiteName = appGroupSuiteName
        statsBroadcastTask = Task.detached(priority: .background) {
            let defaults = suiteName.flatMap { UserDefaults(suiteName: $0) }
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
        // The production fleet is multi-hop only, so fetch the signed
        // multi-hop directory before starting: the actor's resolved config
        // carries it to the FFI, which verifies it against the baked root
        // pin and brings up a MultiHopClient circuit. A failed fetch leaves
        // it nil and the FFI falls back to the legacy single-hop path (dev).
        // Done here (async, off the actor) so the actor stays I/O-free and
        // unit-testable.
        let directory = await Self.fetchMultihopDirectory()
        _actor.multihopDirectoryJSON = directory
        // Persist the directory anti-rollback high-water mark in the App
        // Group container so it survives across connects (iOS has no
        // long-lived daemon to hold it in memory).
        let statePath = multihopGenerationStatePath()
        _actor.multihopGenerationStatePath = statePath
        // Activate exit-pubkey TOFU pinning: the FFI enforces the pin from
        // this App Group file and fails the connection closed on a
        // mismatch (surfaced to the user via the pin-mismatch App Group
        // event below).
        _actor.pinStorePath = pinStorePath()

        // Record the generation this session starts with + arm the periodic
        // refresh so a long-lived session picks up a fleet change (a higher
        // generation) instead of riding a stale circuit until the next manual
        // reconnect. Steady state (unchanged generation) is a no-op fetch.
        if let directory {
            lastDirectoryGeneration = WarrenQuinnAdapter.checkMultihopGeneration(
                directoryJSON: directory,
                generationStatePath: statePath
            ) ?? -1
        }
        warrenSelectedRelays = options.selectedRelays
        startDirectoryRefreshTask(statePath: statePath)

        // Forward to the actor ; the actor decides whether to call
        // `adapter.start(config:)` based on the resolved tunnel config
        // (built from `options` + tunnel settings). No external state
        // observer like WireGuardGo : the Rust-side event callback
        // drives state transitions through `applyEvent`.
        actor.start(options: options)
    }

    /// Resolves the App Group container path for the multi-hop directory
    /// anti-rollback high-water file. Returns nil when the App Group is
    /// unavailable (unit tests), in which case the FFI keeps the rollback
    /// gate per-connect only.
    private func multihopGenerationStatePath() -> String? {
        guard let suite = appGroupSuiteName,
              let container = FileManager.default.containerURL(
                  forSecurityApplicationGroupIdentifier: suite
              )
        else {
            return nil
        }
        return container.appendingPathComponent("warren-multihop-generation").path
    }

    /// Resolves the App Group container path for the exit-pubkey TOFU pin
    /// table (sibling of the multi-hop generation file). Returns nil when
    /// the App Group is unavailable (unit tests), in which case pinning is
    /// off.
    private func pinStorePath() -> String? {
        guard let suite = appGroupSuiteName,
              let container = FileManager.default.containerURL(
                  forSecurityApplicationGroupIdentifier: suite
              )
        else {
            return nil
        }
        return container.appendingPathComponent("warren-exit-pins.json").path
    }

    /// Interval between directory refreshes. Matches the desktop daemon's
    /// 30-minute updater cadence.
    private static let directoryRefreshInterval: UInt64 = 30 * 60 * 1_000_000_000

    /// Arm the periodic directory-refresh loop. Each tick re-fetches the
    /// signed directory and, only when its verified `generation` is strictly
    /// higher than the running session's, re-applies it and reconnects to a
    /// freshly selected circuit. A failed fetch / verify is a no-op (the live
    /// tunnel is never torn down on a transient network blip).
    private func startDirectoryRefreshTask(statePath: String?) {
        directoryRefreshTask?.cancel()
        directoryRefreshTask = Task { [weak self] in
            while !Task.isCancelled {
                try? await Task.sleep(nanoseconds: Self.directoryRefreshInterval)
                guard !Task.isCancelled, let self else { return }
                await self.refreshDirectoryOnce(statePath: statePath)
            }
        }
    }

    private func refreshDirectoryOnce(statePath: String?) async {
        guard let fresh = await Self.fetchMultihopDirectory(),
              let freshGeneration = WarrenQuinnAdapter.checkMultihopGeneration(
                  directoryJSON: fresh,
                  generationStatePath: statePath
              )
        else {
            return
        }
        // Steady state: the fleet did not change, so keep the live circuit.
        guard freshGeneration > lastDirectoryGeneration else { return }
        guard let relays = warrenSelectedRelays else { return }
        logger.info(
            "Warren multi-hop directory generation advanced (\(lastDirectoryGeneration) -> \(freshGeneration)); re-selecting circuit"
        )
        lastDirectoryGeneration = freshGeneration
        _actor.multihopDirectoryJSON = fresh
        _actor.reconnect(to: .preSelected(relays), reconnectReason: .connectionLoss)
    }

    /// warren-api base URL (mirrors the account FFI's baked endpoint).
    private static let warrenApiBaseURL = "https://api.warrenbrowse.com"

    /// Fetches the signed multi-hop directory JSON over URLSession
    /// (transport only; the trust-chain verification happens Rust-side in
    /// the FFI). Returns nil on any non-200 / transport failure so the
    /// caller falls back to the single-hop path rather than blocking the
    /// tunnel start on a dead network.
    private static func fetchMultihopDirectory() async -> String? {
        guard let url = URL(string: "\(warrenApiBaseURL)/v1/multihop/directory") else {
            return nil
        }
        var request = URLRequest(url: url, timeoutInterval: 15)
        request.httpMethod = "GET"
        do {
            let (data, response) = try await URLSession.shared.data(for: request)
            guard let http = response as? HTTPURLResponse, http.statusCode == 200 else {
                return nil
            }
            return String(data: data, encoding: .utf8)
        } catch {
            return nil
        }
    }

    public func stopTunnel() async {
        directoryRefreshTask?.cancel()
        directoryRefreshTask = nil
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

    /// Mirror an exit-pubkey TOFU mismatch into the App Group `UserDefaults`
    /// keys consumed by `WarrenPubKeyWarningPresenter` in the main-app
    /// process. Writes the JSON payload + a timestamp for the freshness
    /// window. A nil `defaults` (unit tests) is a no-op.
    private static func broadcastPinMismatch(
        _ mismatch: WarrenPinMismatch,
        into defaults: UserDefaults?
    ) {
        guard let defaults else { return }
        guard let data = try? JSONEncoder().encode(mismatch),
              let json = String(data: data, encoding: .utf8)
        else {
            return
        }
        defaults.set(json, forKey: WarrenAppGroupKey.pinMismatch.rawValue)
        defaults.set(Date(), forKey: WarrenAppGroupKey.pinMismatchAt.rawValue)
    }
}
