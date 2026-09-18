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

    /// Whether any exit in the roster serves `pubkeyHex`.
    ///
    /// After the user trusts a key, the exit they will reach is the one
    /// advertising it. A key no exit advertises means that exit is gone from
    /// the fleet, which is an ordinary roster change and reads nothing like
    /// the security question the alert just asked.
    func rosterHasExit(servingPubkeyHex pubkeyHex: String) -> Bool {
        guard let wanted = Self.bytes(fromHex: pubkeyHex),
            let cached = try? relayCacheTracker.getCachedRelays()
        else {
            // Nothing to contradict the roster with: say it is still there
            // rather than announce a disappearance on no evidence.
            return true
        }
        return cached.relays.wireguard.relays.contains { $0.publicKey == wanted }
    }

    private static func bytes(fromHex hex: String) -> Data? {
        let characters = Array(hex)
        guard !characters.isEmpty, characters.count.isMultiple(of: 2) else { return nil }
        var bytes = Data(capacity: characters.count / 2)
        for index in stride(from: 0, to: characters.count, by: 2) {
            guard let byte = UInt8(String(characters[index...index + 1]), radix: 16) else {
                return nil
            }
            bytes.append(byte)
        }
        return bytes
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
