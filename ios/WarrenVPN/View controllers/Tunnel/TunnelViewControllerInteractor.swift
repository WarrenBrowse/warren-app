//
//  TunnelViewControllerInteractor.swift
//  MullvadVPN
//
//  Created by pronebird on 26/10/2022.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import Combine
import WarrenSettings
import WarrenTypes

final class TunnelViewControllerInteractor: @unchecked Sendable {
    private let tunnelManager: TunnelManager
    private let relayCacheTracker: RelayCacheTrackerProtocol
    private var tunnelObserver: TunnelObserver?

    var didUpdateTunnelStatus: ((TunnelStatus) -> Void)?
    var didUpdateDeviceState: ((_ deviceState: DeviceState, _ previousDeviceState: DeviceState) -> Void)?
    var didUpdateTunnelSettings: ((LatestTunnelSettings) -> Void)?

    var tunnelStatus: TunnelStatus {
        tunnelManager.tunnelStatus
    }

    var deviceState: DeviceState {
        tunnelManager.deviceState
    }

    var tunnelSettings: LatestTunnelSettings {
        tunnelManager.settings
    }

    /// Coexistence: whether this build has stood down for a higher-priority
    /// product environment, from the same record the tunnel refuses on.
    var isStandingDownForHigherEnvironment: Bool {
        tunnelManager.envStandDownRecord.isStandingDown
    }

    init(
        tunnelManager: TunnelManager,
        relayCacheTracker: RelayCacheTrackerProtocol
    ) {
        self.tunnelManager = tunnelManager
        self.relayCacheTracker = relayCacheTracker

        let tunnelObserver = TunnelBlockObserver(
            didUpdateTunnelStatus: { [weak self] _, tunnelStatus in
                self?.didUpdateTunnelStatus?(tunnelStatus)
            },
            didUpdateDeviceState: { [weak self] _, deviceState, previousDeviceState in
                self?.didUpdateDeviceState?(deviceState, previousDeviceState)
            },
            didUpdateTunnelSettings: { [weak self] _, tunnelSettings in
                self?.didUpdateTunnelSettings?(tunnelSettings)
            }
        )

        tunnelManager.addObserver(tunnelObserver)

        self.tunnelObserver = tunnelObserver
    }

    func startTunnel() {
        tunnelManager.startTunnel()
    }

    func stopTunnel() {
        tunnelManager.stopTunnel()
    }

    func reconnectTunnel(selectNewRelay: Bool) {
        tunnelManager.reconnectTunnel(selectNewRelay: selectNewRelay)
    }

    /// Desktop-parity shuffle: pick a random exit country among those
    /// with an active relay, pin the exit constraint to it and
    /// (re)connect. Mirrors the desktop ShuffleButton semantics, which
    /// randomizes at country granularity and lets the relay selector
    /// pick the concrete relay within it.
    func shuffleExitLocation() {
        guard let cachedRelays = try? relayCacheTracker.getCachedRelays() else { return }
        let countries = Set(
            cachedRelays.relays.wireguard.relays
                .filter { $0.active }
                .map { $0.location.country }
        )
        guard let pick = countries.randomElement() else { return }

        var relayConstraints = tunnelManager.settings.relayConstraints
        relayConstraints.exitLocations = .only(UserSelectedRelays(locations: [.country(pick)]))

        tunnelManager.updateSettings([.relayConstraints(relayConstraints)]) { [weak self] in
            self?.tunnelManager.startTunnel()
        }
    }
}
