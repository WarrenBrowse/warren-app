//
//  StorePaymentEvent.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import StoreKit
import WarrenRustRuntime

enum StorePaymentEvent {
    /// Successful payment
    case successfulPayment(StorePaymentOutcome)
    /// User cancelled the purchase
    case userCancelled
    /// Payment was made but it is still being processed. This transaction
    /// can be processed and the receipt uploaded to the API later, when the
    /// transaction listener handles it.
    case pending
    /// Purchasing failed
    case failed(StorePaymentError)
}

enum StorePaymentError: Error {
    /// Purchase failed because the product being purchased is either
    /// unavailable or StoreKit services failed.
    case storeKitError(StoreKitError)
    /// Purchase failed because of a "purchase error".
    case purchaseError(Product.PurchaseError)
    /// User made a purchase, but we failed to verify the transaction. In
    /// this case, it is fine to not send the transaction to the API.
    case verification(VerificationResult<Transaction>.VerificationError)
    /// The user initiated the payment but the app failed to fetch a payment
    /// token from the API. No money has been spent and the payment failed.
    case getPaymentToken(Error)
    /// The user already spent money but we failed to upload the receipt to
    /// the API. The receipt can be uploaded again later.
    case receiptUpload
    /// The wallet is banned, so warren-api opened no payment session and
    /// StoreKit was never asked to charge (warren-core doc 105 section 5.3).
    case bannedBeforePayment(WarrenAccountBan)
    /// The wallet is banned, so warren-api left the paid transaction
    /// unclaimed. It stays unfinished: StoreKit presents it again at every
    /// start, and it credits the wallet once the ban ends.
    case bannedAfterPayment(WarrenAccountBan)
    /// Purchase restoration was unsuccessful.
    case restorationError
    /// StoreKit returned no purchasable products (no store account, region
    /// restrictions, or store outage); the web checkout still works.
    case productsUnavailable
    /// Fallback for errors we do not recognize.
    case unknown

    var description: String {
        switch self {
        case let .storeKitError(error):
            error.localizedDescription
        case let .purchaseError(error):
            error.localizedDescription
        case .verification:
            NSLocalizedString("Failed to verify transaction receipt", comment: "")
        case .getPaymentToken:
            NSLocalizedString("Failed to reach Warren servers to initiate purchase", comment: "")
        case .receiptUpload:
            NSLocalizedString(
                "Failed to upload one or more receipts to Warren servers. Try again later or contact support for help.",
                comment: ""
            )
        case let .bannedBeforePayment(ban):
            WarrenAccountStandingText.ban(ban) + " "
                + String(localized: "No payment was taken.", table: "Settings")
        case let .bannedAfterPayment(ban):
            WarrenAccountStandingText.ban(ban) + " "
                + String(
                    localized: "Your purchase is kept and will be credited once the revocation ends.",
                    table: "Settings"
                )
        case .restorationError:
            NSLocalizedString(
                "Could not restore previous purchases. Try again later or contact support.",
                comment: ""
            )
        case .productsUnavailable:
            NSLocalizedString(
                "In-app purchases are unavailable right now. You can pay by card on the web and use the voucher you receive.",
                comment: ""
            )
        case .unknown:
            NSLocalizedString("Unexpected error occured.", comment: "")
        }
    }
}

extension StorePaymentError {
    /// The failure a payment token request ends the purchase with, before
    /// StoreKit is asked for anything.
    static func ofTokenFailure(_ error: Error) -> StorePaymentError {
        if let ban = bannedAccount(error) {
            return .bannedBeforePayment(ban)
        }
        return .getPaymentToken(error)
    }

    /// The failure a receipt upload ends a paid purchase with. Either way the
    /// transaction stays unfinished, for the next start to present again.
    static func ofUploadFailure(_ error: Error) -> StorePaymentError {
        if let ban = bannedAccount(error) {
            return .bannedAfterPayment(ban)
        }
        return .receiptUpload
    }

    private static func bannedAccount(_ error: Error) -> WarrenAccountBan? {
        guard case let .account(.banned(ban)) = error as? WarrenWalletInteractorError else {
            return nil
        }
        return ban
    }
}
