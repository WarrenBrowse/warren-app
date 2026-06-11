//
//  RedeemVoucherInteractor.swift
//  MullvadVPN
//
//  Created by Mojgan on 2023-08-30.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import Foundation
import WarrenREST
import WarrenTypes

final class RedeemVoucherInteractor: @unchecked Sendable {
    private let tunnelManager: TunnelManager

    private var tasks: [Cancellable] = []

    // The logout dialog only ever fired when a redeemed code was recognized as
    // a Mullvad account number, a path Warren no longer exercises (the wallet
    // model has no account-number login). These hooks stay so the view layer
    // compiles, but `showLogoutDialog` is never invoked.
    var showLogoutDialog: (() -> Void)?
    var didLogout: ((String) -> Void)?

    init(tunnelManager: TunnelManager) {
        self.tunnelManager = tunnelManager
    }

    func redeemVoucher(
        code: String,
        completion: @escaping (@Sendable (Result<REST.SubmitVoucherResponse, Error>) -> Void)
    ) {
        tasks.append(
            tunnelManager.redeemVoucher(code) { result in
                completion(result)
            })
    }

    func logout() async {
        await MainActor.run {
            WarrenWalletLogout.perform(tunnelManager: tunnelManager)
        }
    }

    func cancelAll() {
        tasks.forEach { $0.cancel() }
    }
}
