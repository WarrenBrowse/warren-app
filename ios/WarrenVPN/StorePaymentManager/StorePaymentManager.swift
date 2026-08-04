//
//  StorePaymentManager.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import StoreKit
import WarrenLogging
import WarrenTypes

/// Manager responsible for handling App Store payments and passing
/// StoreKit receipts to the Warren backend (warren-api), which credits
/// the wallet's subscription.
///
/// Actor isolation serializes all access; callers interact via `await`
/// from any executor. Observers are notified on the main actor.
final actor StorePaymentManager {
    private let logger = Logger(label: "StorePaymentManager")
    private var observerList = ObserverList<StorePaymentObserver>()
    private let interactor: StorePaymentManagerInteractor
    private var processedTransactionIds: Set<UInt64> = []
    private var updateListenerTask: Task<Void, Never>?

    /// Designated initializer.
    ///
    /// - Parameter interactor: bridges StoreKit to warren-api via the
    ///   wallet.
    init(interactor: StorePaymentManagerInteractor) {
        self.interactor = interactor
    }

    /// Start listening for transaction updates.
    func start() async {
        logger.debug("Starting StoreKit transaction listener")

        #if !DEBUG
            // Always clean up non-production transactions immediately. If
            // old unfinished sandbox transactions spilled over from
            // TestFlight, they would clog the pipeline since they can never
            // be finished or removed in production.
            await Self.finishOutstandingSandboxAndOldAPITransactions()
        #endif

        _ = try? await processOutstandingTransactions()

        updateListenerTask?.cancel()
        updateListenerTask = Task { [weak self] in
            guard let self else { return }

            for await verification in Transaction.updates {
                guard await shouldProcessPayment(verification: verification) else {
                    continue
                }

                // A transaction landing here did not go through
                // `purchase()`: a deferred Ask to Buy approval or a
                // purchase finalized outside the in-app flow. Refreshing
                // account data alone would leave it unfinished and
                // uncredited until the next app start, so upload the
                // receipt and finish it now. On upload failure the
                // transaction stays in `Transaction.unfinished` and is
                // retried by `processOutstandingTransactions` at next
                // start.
                await self.processTransactionUpdate(verification)
            }
        }
    }

    // MARK: Notifications

    func addPaymentObserver(_ observer: StorePaymentObserver) {
        observerList.append(observer)
    }

    // MARK: - Products and payments

    func products() async throws -> [Product] {
        try await Product.products(for: StoreSubscription.allCases.map { $0.rawValue })
    }

    func purchase(product: Product) async {
        logger.debug("Purchasing product: \(product.id)")

        let token: UUID
        do {
            token = try await self.getPaymentToken()
        } catch {
            didFailFetchingToken(error: error)
            return
        }

        let result: Product.PurchaseResult
        do {
            result = try await product.purchase(
                options: [.appAccountToken(token)]
            )
        } catch {
            didFailPurchase(error: error)
            return
        }

        switch result {
        case let .success(.verified(transaction)):
            await purchaseWasSuccessful(transaction: transaction)
        case let .success(.unverified(transaction, verificationFailure)):
            await didFailVerification(transaction: transaction, error: verificationFailure)
        case .userCancelled:
            userDidCancel()
        case .pending:
            didSuspendPurchase()
        @unknown default:
            // A future StoreKit PurchaseResult case must not crash a
            // user who has just paid: degrade to a generic failure so the
            // UI can recover instead of hitting fatalError post-charge.
            logger.error("Unhandled purchase result: \(result)")
            notifyObservers(of: .failed(.unknown))
        }
    }

    func processOutstandingTransactions() async throws -> StorePaymentOutcome {
        var timeAdded: TimeInterval = 0
        var failedOneOrMoreTransactions = false

        logger.debug("Processing outstanding transactions")

        for await verification in Transaction.unfinished {
            guard shouldProcessPayment(verification: verification) else {
                continue
            }

            do {
                try await uploadReceipt(verification: verification)
            } catch {
                failedOneOrMoreTransactions = true
                continue
            }

            let payload = try verification.payloadValue
            await payload.finish()

            addToProcessedTransactions(verification)
            timeAdded += timeFromProduct(id: payload.productID)
        }

        await updateAccountData()

        if failedOneOrMoreTransactions {
            throw StorePaymentError.receiptUpload
        }

        return if timeAdded > 0 {
            .timeAdded(timeAdded)
        } else {
            .noTimeAdded
        }
    }

    static func finishOutstandingSandboxAndOldAPITransactions() async {
        let logger = Logger(label: "StorePaymentManager")

        logger.debug("Finishing outstanding sandbox and old transactions")

        for await verification in Transaction.unfinished {
            guard let payload = try? verification.payloadValue else {
                logger.debug("Verification is missing a valid payload")
                continue
            }

            logger.debug("Unfinished transaction environment is \(payload.environment)")

            let isStagingEnvironment = payload.environment != .production
            let isOldAPI = !StoreSubscription.allCases
                .map { $0.rawValue }
                .contains(payload.productID)

            if isStagingEnvironment || isOldAPI {
                logger.debug(
                    "Finishing transaction. isStagingEnvironment: \(isStagingEnvironment), isOldAPI: \(isOldAPI)"
                )
                await payload.finish()
            } else {
                logger.debug(
                    "Skipping transaction. isStagingEnvironment: \(isStagingEnvironment), isOldAPI: \(isOldAPI)"
                )
            }
        }
    }

    // MARK: - Private methods

    /// Credits a transaction delivered by `Transaction.updates`: upload
    /// the receipt to warren-api, finish it, then refresh the wallet
    /// expiry. Observers are not notified here: the concurrent
    /// `purchase()` flow owns the user-facing outcome for direct
    /// purchases, and out-of-band credits surface through the account
    /// data refresh.
    private func processTransactionUpdate(_ verification: VerificationResult<Transaction>) async {
        do {
            try await uploadReceipt(verification: verification)

            let payload = try verification.payloadValue
            await payload.finish()

            addToProcessedTransactions(verification)
        } catch {
            logger.error("Failed to process a transaction update; will retry at next start")
        }

        await updateAccountData()
    }

    private func getPaymentToken() async throws -> UUID {
        let result = await interactor.initPayment()

        switch result {
        case let .success(token): return token
        case let .failure(error): throw error
        }
    }

    private func uploadReceipt(verification: VerificationResult<Transaction>) async throws {
        let payload = try verification.payloadValue

        let logMessage: String =
            "Uploading receipt. "
            + "Product ID: \(payload.productID), "
            + "Environment: \(payload.environment), "
            + "Purchase date: \(payload.purchaseDate.safeLogFormatted), "
            + "Revocation date: \(payload.revocationDate?.safeLogFormatted ?? "none")"
        logger.debug(.init(stringLiteral: logMessage))

        let result = await interactor.checkPayment(jwsRepresentation: verification.jwsRepresentation)

        switch result {
        case .success: return
        case let .failure(error): throw error
        }
    }

    private func purchaseWasSuccessful(transaction: Transaction) async {
        let verification = VerificationResult<Transaction>.verified(transaction)

        do {
            try await uploadReceipt(verification: verification)
            await updateAccountData()

            try await verification.payloadValue.finish()

            addToProcessedTransactions(verification)
            didPurchaseMoreTime(outcome: .timeAdded(timeFromProduct(id: transaction.productID)))
        } catch {
            didFailUploadingReceipt()
        }
    }

    /// Refreshes the wallet-backed subscription expiry after a credit so
    /// the UI reflects the new time. The wallet interactor is the source
    /// of truth; warren-api returns the new expiry.
    private func updateAccountData() async {
        logger.debug("Updating account data")
        await interactor.updateAccountData()
    }

    private func transactionHasBeenProcessed(_ verificationResult: VerificationResult<Transaction>) -> Bool {
        guard let transactionId = try? verificationResult.payloadValue.id else {
            return true
        }

        let hasAlreadyBeenProcessed = processedTransactionIds.contains(transactionId)
        if hasAlreadyBeenProcessed {
            logger.debug("Verification has already been processed")
        }

        return hasAlreadyBeenProcessed
    }

    private func addToProcessedTransactions(_ verificationResult: VerificationResult<Transaction>) {
        guard let transactionId = try? verificationResult.payloadValue.id else {
            return
        }

        logger.debug("Adding to processed transactions")

        _ = processedTransactionIds.insert(transactionId)
    }

    /// Returns time added, in seconds, for a product ID.
    private func timeFromProduct(id: String) -> TimeInterval {
        guard let product = StoreSubscription(rawValue: id) else { return 0 }
        return Duration.days(product.months * 30).timeInterval
    }

    private func shouldProcessPayment(verification: VerificationResult<Transaction>) -> Bool {
        guard case VerificationResult<Transaction>.verified = verification else {
            logger.debug("Verification was not .verified, instead was: \(verification)")
            return false
        }

        if let revocationDate = try? verification.payloadValue.revocationDate {
            logger.debug("Verification was revoked at: \(revocationDate)")
            return false
        }

        return !transactionHasBeenProcessed(verification)
    }

    // MARK: Notifications

    private func didPurchaseMoreTime(outcome: StorePaymentOutcome) {
        logger.debug("Purchase successful")
        notifyObservers(of: .successfulPayment(outcome))
    }

    private func userDidCancel() {
        logger.debug("User cancelled purchase")
        notifyObservers(of: .userCancelled)
    }

    private func didSuspendPurchase() {
        logger.debug("Did suspend purchase")
        notifyObservers(of: .pending)
    }

    private func didFailFetchingToken(error: Error) {
        logger.debug("Did fail fetching token, with error: \(error)")
        notifyObservers(of: .failed(.getPaymentToken(error)))
    }

    private func didFailUploadingReceipt() {
        logger.debug("Did fail uploading receipt")
        notifyObservers(of: .failed(.receiptUpload))
    }

    private func didFailVerification(
        transaction: Transaction,
        error: VerificationResult<Transaction>.VerificationError
    ) async {
        await transaction.finish()

        logger.debug("Did fail verification, with error: \(error)")
        notifyObservers(of: .failed(.verification(error)))
    }

    private func didFailPurchase(error: Error) {
        let failure: StorePaymentError
        switch error {
        case let storeKitError as StoreKitError:
            failure = .storeKitError(storeKitError)

        case let purchaseError as Product.PurchaseError:
            failure = .purchaseError(purchaseError)

        default:
            failure = .unknown
        }

        logger.debug("Did fail purchase, with error: \(error)")
        notifyObservers(of: .failed(failure))
    }

    private func notifyObservers(of storeKitEvent: StorePaymentEvent) {
        observerList.notify { observer in
            Task { @MainActor in
                observer.storePaymentManager(didReceiveEvent: storeKitEvent)
            }
        }
    }
}
