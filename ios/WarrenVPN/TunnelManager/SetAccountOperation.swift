//
//  SetAccountOperation.swift
//  MullvadVPN
//
//  Created by pronebird on 16/12/2021.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import Foundation
import WarrenLogging
import WarrenSettings
import WarrenTypes
import Operations

enum SetAccountAction {
    /// Unset account.
    case unset(isRemovingProfile: Bool)

    var taskName: String {
        switch self {
        case .unset: "Unset account"
        }
    }
}

// Warren identifies users by wallet pubkey. Account creation, retrieval, and
// deletion run through `WarrenAccountClient` (the warren-api wallet client), so
// this operation only handles logging out (resetting the device state and
// removing the VPN configuration). Desktop and Android are aligned.
class SetAccountOperation: ResultOperation<StoredAccountData?>, @unchecked Sendable {
    private let interactor: TunnelInteractor
    private let action: SetAccountAction

    private let logger = Logger(label: "SetAccountOperation")

    init(
        dispatchQueue: DispatchQueue,
        interactor: TunnelInteractor,
        action: SetAccountAction
    ) {
        self.interactor = interactor
        self.action = action

        super.init(dispatchQueue: dispatchQueue)
    }

    // MARK: -

    override func main() {
        switch action {
        case .unset(let isRemovingProfile):
            startLogoutFlow(isRemovingProfile: isRemovingProfile) { [self] in
                finish(result: .success(nil))
            }
        }
    }

    // MARK: - Private

    /**
     Begin logout flow by transitioning the device state to logged out, removing
     the system VPN configuration if any, and resetting the tunnel status. Does
     nothing if already logged out.
     */
    private func startLogoutFlow(isRemovingProfile: Bool = true, completion: @escaping @Sendable () -> Void) {
        switch interactor.deviceState {
        case .loggedIn, .revoked:
            unsetDeviceState(isRemovingProfile: isRemovingProfile, completion: completion)

        case .loggedOut:
            completion()
        }
    }

    /**
     Transitions device state into logged out state by performing the following tasks:

     1. Prepare tunnel manager for removal of VPN configuration. In response tunnel manager stops processing VPN status
        notifications coming from VPN configuration.
     2. Reset device state to logged out and persist it.
     3. Remove VPN configuration and release an instance of `Tunnel` object.
     */
    private func unsetDeviceState(isRemovingProfile: Bool = true, completion: @escaping @Sendable () -> Void) {
        // Reset tunnel and device state.
        interactor.updateTunnelStatus { tunnelStatus in
            tunnelStatus = TunnelStatus()
            tunnelStatus.state = .disconnected
        }
        interactor.setDeviceState(.loggedOut, persist: true)

        // Finish immediately if tunnel provider is not set.
        guard let tunnel = interactor.tunnel, isRemovingProfile else {
            completion()
            return
        }

        // Tell the caller to unsubscribe from VPN status notifications.
        interactor.prepareForVPNConfigurationDeletion()

        // Remove VPN configuration.
        tunnel.removeFromPreferences { [self] error in
            dispatchQueue.async { [self] in
                // Ignore error but log it.
                if let error {
                    logger.error(error: error, message: "Failed to remove VPN configuration.")
                }

                interactor.setTunnel(nil, shouldRefreshTunnelState: false)

                completion()
            }
        }
    }
}
