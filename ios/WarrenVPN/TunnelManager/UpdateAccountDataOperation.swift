//
//  UpdateAccountDataOperation.swift
//  MullvadVPN
//
//  Created by pronebird on 12/05/2022.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import Foundation
import WarrenLogging
import WarrenRustRuntime
import WarrenSettings
import WarrenTypes
import Operations

/// Refreshes the subscription expiry on the synthesized wallet-backed
/// device state. The expiry now comes from warren-api
/// (`GET /v1/subscription`, signed with the wallet key) instead of the
/// Mullvad account-number REST endpoint, matching desktop and Android.
class UpdateAccountDataOperation: ResultOperation<Void>, @unchecked Sendable {
    private let logger = Logger(label: "UpdateAccountDataOperation")
    private let interactor: TunnelInteractor
    private let walletInteractor: WarrenWalletInteractor

    init(
        dispatchQueue: DispatchQueue,
        interactor: TunnelInteractor,
        walletInteractor: WarrenWalletInteractor
    ) {
        self.interactor = interactor
        self.walletInteractor = walletInteractor

        super.init(dispatchQueue: dispatchQueue)
    }

    override func main() {
        guard case .loggedIn = interactor.deviceState else {
            finish(result: .failure(InvalidDeviceStateError()))
            return
        }

        walletInteractor.fetchSubscriptionExpiry { [weak self] result in
            guard let self else { return }
            self.dispatchQueue.async {
                self.didReceiveExpiry(result: result)
            }
        }
    }

    private func didReceiveExpiry(result: Result<Date, WarrenWalletInteractorError>) {
        guard !isCancelled else {
            finish(result: .failure(OperationError.cancelled))
            return
        }

        let mapped = result.tryMap { expiry in
            switch interactor.deviceState {
            case .loggedIn(var storedAccountData, let storedDeviceData):
                storedAccountData.expiry = expiry
                interactor.setDeviceState(.loggedIn(storedAccountData, storedDeviceData), persist: true)
            default:
                throw InvalidDeviceStateError()
            }
        }.inspectError { error in
            self.logger.error(error: error, message: "Failed to refresh subscription expiry.")
        }

        finish(result: mapped)
    }
}
