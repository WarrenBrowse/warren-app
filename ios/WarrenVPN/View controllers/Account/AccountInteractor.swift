//
//  AccountInteractor.swift
//  MullvadVPN
//
//  Created by pronebird on 26/10/2022.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import Foundation
import WarrenREST
import WarrenSettings
import WarrenTypes
import Operations

final class AccountInteractor: Sendable {
    let tunnelManager: TunnelManager
    let accountsProxy: RESTAccountHandling

    nonisolated(unsafe) var didReceiveTunnelState: (() -> Void)?
    nonisolated(unsafe) var didReceiveDeviceState: (@Sendable (DeviceState) -> Void)?

    nonisolated(unsafe) private var tunnelObserver: TunnelObserver?

    init(tunnelManager: TunnelManager, accountsProxy: RESTAccountHandling) {
        self.tunnelManager = tunnelManager
        self.accountsProxy = accountsProxy

        let tunnelObserver =
            TunnelBlockObserver(
                didUpdateTunnelStatus: { [weak self] _, _ in
                    self?.didReceiveTunnelState?()
                },
                didUpdateDeviceState: { [weak self] _, deviceState, _ in
                    self?.didReceiveDeviceState?(deviceState)
                }
            )

        tunnelManager.addObserver(tunnelObserver)

        self.tunnelObserver = tunnelObserver
    }

    var tunnelState: TunnelState {
        tunnelManager.tunnelStatus.state
    }

    var deviceState: DeviceState {
        tunnelManager.deviceState
    }

    /// The Warren wallet SS58 identity shown in place of the Mullvad account
    /// number, or nil when no wallet is provisioned.
    var walletAddress: String? {
        WarrenWalletInteractor().publicKeyAddress()
    }

    func logout() async {
        await tunnelManager.unsetAccount()
        await MainActor.run {
            WarrenWalletLogout.perform(tunnelManager: tunnelManager)
        }
    }
}
