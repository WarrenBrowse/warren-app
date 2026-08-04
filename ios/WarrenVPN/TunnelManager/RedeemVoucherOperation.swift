//
//  RedeemVoucherOperation.swift
//  MullvadVPN
//
//  Created by pronebird on 29/03/2023.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import Foundation
import WarrenLogging
import WarrenREST
import WarrenRustRuntime
import WarrenSettings
import WarrenTypes
import Operations

/// Redeems a subscription voucher against warren-api
/// (`POST /v1/register`, unsigned, pubkey in body) instead of the Mullvad
/// voucher REST endpoint. The wallet pubkey is the account; redeeming a
/// voucher binds it to a new subscription and refreshes the expiry on the
/// synthesized device state.
class RedeemVoucherOperation: ResultOperation<REST.SubmitVoucherResponse>, @unchecked Sendable {
    private let logger = Logger(label: "RedeemVoucherOperation")
    private let interactor: TunnelInteractor
    private let walletInteractor: WarrenWalletInteractor
    private let voucherCode: String

    init(
        dispatchQueue: DispatchQueue,
        interactor: TunnelInteractor,
        voucherCode: String,
        walletInteractor: WarrenWalletInteractor
    ) {
        self.interactor = interactor
        self.voucherCode = voucherCode
        self.walletInteractor = walletInteractor

        super.init(dispatchQueue: dispatchQueue)
    }

    override func main() {
        guard case let .loggedIn(accountData, _) = interactor.deviceState else {
            finish(result: .failure(InvalidDeviceStateError()))
            return
        }
        let previousExpiry = accountData.expiry

        walletInteractor.redeemVoucher(code: voucherCode) { [weak self] result in
            guard let self else { return }
            self.dispatchQueue.async {
                self.didReceiveExpiry(result: result, previousExpiry: previousExpiry)
            }
        }
    }

    private func didReceiveExpiry(
        result: Result<Date, WarrenWalletInteractorError>,
        previousExpiry: Date
    ) {
        guard !isCancelled else {
            finish(result: .failure(OperationError.cancelled))
            return
        }

        let mapped: Result<REST.SubmitVoucherResponse, Error> = result
            .mapError(Self.mapVoucherError)
            .tryMap { newExpiry in
                switch interactor.deviceState {
                case .loggedIn(var storedAccountData, let storedDeviceData):
                    storedAccountData.expiry = newExpiry
                    interactor.setDeviceState(.loggedIn(storedAccountData, storedDeviceData), persist: true)
                    // "Time added" is measured from whichever is later: the
                    // previous expiry (renewal) or now (a lapsed account).
                    let base = max(previousExpiry, Date())
                    let added = max(0, Int(newExpiry.timeIntervalSince(base)))
                    return REST.SubmitVoucherResponse(timeAdded: added, newExpiry: newExpiry)
                default:
                    throw InvalidDeviceStateError()
                }
            }.inspectError { error in
                self.logger.error(error: error, message: "Failed to redeem voucher.")
            }

        finish(result: mapped)
    }

    /// Maps the warren-api voucher failure to a user-facing localized
    /// error. The HTTP status carries the rejection reason; the response
    /// body is never surfaced.
    private static func mapVoucherError(_ error: WarrenWalletInteractorError) -> Error {
        guard case let .account(accountError) = error,
              case let .server(status, _) = accountError else {
            return VoucherRedemptionError(message: NSLocalizedString(
                "Could not redeem the voucher. Please check your connection and try again.",
                tableName: "Wallet",
                comment: "Voucher redemption failed for a transport reason"
            ))
        }
        let message: String
        switch status {
        case 400:
            message = NSLocalizedString(
                "This voucher is invalid.",
                tableName: "Wallet",
                comment: "Voucher redemption rejected: malformed or unknown voucher"
            )
        case 409:
            message = NSLocalizedString(
                "This voucher has already been redeemed.",
                tableName: "Wallet",
                comment: "Voucher redemption rejected: voucher already used"
            )
        case 410:
            message = NSLocalizedString(
                "This voucher has been cancelled.",
                tableName: "Wallet",
                comment: "Voucher redemption rejected: voucher cancelled by an admin"
            )
        case 429:
            message = NSLocalizedString(
                "Too many attempts. Please wait a moment and try again.",
                tableName: "Wallet",
                comment: "Voucher redemption rate limited"
            )
        default:
            message = NSLocalizedString(
                "Could not redeem the voucher. Please try again.",
                tableName: "Wallet",
                comment: "Voucher redemption failed for an unexpected reason"
            )
        }
        return VoucherRedemptionError(message: message)
    }
}

/// A localized voucher-redemption failure surfaced to the redeem-voucher
/// screen via `error.localizedDescription`.
private struct VoucherRedemptionError: LocalizedError {
    let message: String
    var errorDescription: String? { message }
}
