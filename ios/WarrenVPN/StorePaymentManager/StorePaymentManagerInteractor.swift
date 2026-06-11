//
//  StorePaymentManagerInteractor.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import Foundation
import WarrenRustRuntime

/// Bridges the StoreKit payment flow to the Warren wallet backend.
///
/// Warren has no Mullvad account number: the identity is the BIP39
/// wallet. This interactor therefore talks to warren-api through
/// `WarrenWalletInteractor` (signed requests with the wallet key)
/// instead of the deleted Mullvad `apiProxy` / `accountProxy`.
///
/// - `initPayment()` mints an ephemeral payment session bound to the
///   wallet (signed `POST /v1/payments/apple/init`) and returns the
///   session UUID to pass to StoreKit as the `appAccountToken`. The
///   backend resolves that token back to the wallet at check time, so
///   Apple never sees the pubkey.
/// - `checkPayment(jwsRepresentation:)` uploads the StoreKit 2 signed
///   transaction JWS (signed `POST /v1/payments/apple/check`); the
///   backend verifies it against Apple's root CA and credits the
///   wallet's subscription.
final actor StorePaymentManagerInteractor {
    private let tunnelManager: TunnelManager
    private let walletInteractor: WarrenWalletInteractor

    init(tunnelManager: TunnelManager, walletInteractor: WarrenWalletInteractor = WarrenWalletInteractor()) {
        self.tunnelManager = tunnelManager
        self.walletInteractor = walletInteractor
    }

    /// Refreshes the wallet-backed device-state expiry from warren-api.
    /// Reuses the same path the subscription fetch and voucher redeem
    /// already use, so the StoreKit credit surfaces in the UI exactly
    /// like any other top-up.
    func updateAccountData() async {
        await withCheckedContinuation { continuation in
            tunnelManager.updateAccountData { _ in
                continuation.resume()
            }
        }
    }

    /// Mints a StoreKit payment token (the `appAccountToken`).
    func initPayment() async -> Result<UUID, Error> {
        await withCheckedContinuation { continuation in
            walletInteractor.storeKitInitPayment { result in
                switch result {
                case let .success(token):
                    if let uuid = UUID(uuidString: token) {
                        continuation.resume(returning: .success(uuid))
                    } else {
                        continuation.resume(
                            returning: .failure(StorePaymentError.unknown)
                        )
                    }
                case let .failure(error):
                    continuation.resume(returning: .failure(error))
                }
            }
        }
    }

    /// Uploads the StoreKit 2 signed transaction JWS so the backend can
    /// verify it and credit the wallet.
    func checkPayment(jwsRepresentation: String) async -> Result<Void, Error> {
        await withCheckedContinuation { continuation in
            walletInteractor.submitStoreKitTransaction(jws: jwsRepresentation) { result in
                switch result {
                case .success:
                    continuation.resume(returning: .success(()))
                case let .failure(error):
                    continuation.resume(returning: .failure(error))
                }
            }
        }
    }
}
