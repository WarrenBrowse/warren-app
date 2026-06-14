//
//  StorePaymentManagerInteractorTests.swift
//  WarrenVPNTests
//
//  Created by Warren on 2026-06-14.
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  Tests the testable logic of `StorePaymentManagerInteractor`:
//    - `initPayment()` maps the backend token string to a `UUID`
//      (success) or `.failure(.unknown)` when the string is not a UUID,
//      and propagates a wallet/backend failure unchanged.
//    - `checkPayment(jwsRepresentation:)` maps wallet success/failure to
//      `Result<Void, Error>`.
//
//  The interactor depends on the concrete `WarrenWalletInteractor`, which
//  hits the Keychain + warren-api and is therefore not unit-testable.
//  These tests drive it through the `WarrenWalletStoreKitInteracting`
//  seam with an in-memory mock. `updateAccountData()` is NOT covered:
//  it depends on the concrete `TunnelManager` (5 collaborators, no seam),
//  and the StoreKit-to-UUID mapping is the security-relevant logic.
//

import XCTest
@testable import WarrenVPN

@testable import WarrenMockData
@testable import WarrenREST
@testable import WarrenRustRuntime
@testable import WarrenSettings
@testable import WarrenTypes

/// In-memory stand-in for `WarrenWalletInteractor` driving the
/// StoreKit seam. Returns canned results so the interactor's mapping
/// logic can be asserted without the Keychain / warren-api round-trip.
private final class MockWalletStoreKitInteractor: WarrenWalletStoreKitInteracting, @unchecked Sendable {
    var initResult: Result<String, WarrenWalletInteractorError>
    var checkResult: Result<Date, WarrenWalletInteractorError>
    private(set) var lastSubmittedJWS: String?

    init(
        initResult: Result<String, WarrenWalletInteractorError> = .success(UUID().uuidString),
        checkResult: Result<Date, WarrenWalletInteractorError> = .success(Date())
    ) {
        self.initResult = initResult
        self.checkResult = checkResult
    }

    func storeKitInitPayment(
        completion: @escaping @Sendable (Result<String, WarrenWalletInteractorError>) -> Void
    ) {
        completion(initResult)
    }

    func submitStoreKitTransaction(
        jws: String,
        completion: @escaping @Sendable (Result<Date, WarrenWalletInteractorError>) -> Void
    ) {
        lastSubmittedJWS = jws
        completion(checkResult)
    }
}

final class StorePaymentManagerInteractorTests: XCTestCase {
    static let store = InMemorySettingsStore<SettingNotFound>()

    override static func setUp() {
        SettingsManager.unitTestStore = store
    }

    override static func tearDown() {
        store.reset()
    }

    /// Builds a `TunnelManager` only to satisfy the interactor's init.
    /// None of the tests below call `updateAccountData()`, so the manager
    /// is never exercised; it mirrors the stub wiring in TunnelManagerTests.
    private func makeTunnelManager() -> TunnelManager {
        TunnelManager(
            backgroundTaskProvider: UIApplicationStub(),
            tunnelStore: TunnelStore(application: UIApplicationStub()),
            relayCacheTracker: RelayCacheTrackerStub(),
            apiProxy: APIProxyStub(),
            relaySelector: RelaySelectorStub { _ in
                try RelaySelectorStub.nonFallible().selectRelays(
                    tunnelSettings: LatestTunnelSettings(),
                    connectionAttemptCount: 0
                )
            }
        )
    }

    private func makeInteractor(
        wallet: MockWalletStoreKitInteractor
    ) -> StorePaymentManagerInteractor {
        StorePaymentManagerInteractor(
            tunnelManager: makeTunnelManager(),
            walletInteractor: wallet
        )
    }

    // MARK: - initPayment

    func test_initPayment_returnsUUID_whenBackendTokenIsValidUUID() async {
        let expected = UUID()
        let wallet = MockWalletStoreKitInteractor(initResult: .success(expected.uuidString))
        let interactor = makeInteractor(wallet: wallet)

        let result = await interactor.initPayment()

        switch result {
        case let .success(uuid):
            XCTAssertEqual(uuid, expected)
        case let .failure(error):
            XCTFail("Expected success, got failure: \(error)")
        }
    }

    func test_initPayment_failsUnknown_whenBackendTokenIsNotAUUID() async {
        let wallet = MockWalletStoreKitInteractor(initResult: .success("not-a-uuid"))
        let interactor = makeInteractor(wallet: wallet)

        let result = await interactor.initPayment()

        switch result {
        case .success:
            XCTFail("A non-UUID token must not be accepted as a payment token")
        case let .failure(error):
            // StorePaymentError is not Equatable; match the case directly.
            guard case StorePaymentError.unknown = error else {
                return XCTFail("Expected StorePaymentError.unknown, got \(error)")
            }
        }
    }

    func test_initPayment_failsUnknown_whenBackendTokenIsEmpty() async {
        let wallet = MockWalletStoreKitInteractor(initResult: .success(""))
        let interactor = makeInteractor(wallet: wallet)

        let result = await interactor.initPayment()

        if case .success = result {
            XCTFail("Empty token must not parse as a UUID")
        }
    }

    func test_initPayment_propagatesWalletFailure() async {
        let wallet = MockWalletStoreKitInteractor(initResult: .failure(.noWallet))
        let interactor = makeInteractor(wallet: wallet)

        let result = await interactor.initPayment()

        switch result {
        case .success:
            XCTFail("Expected the wallet failure to propagate")
        case let .failure(error):
            XCTAssertEqual(error as? WarrenWalletInteractorError, .noWallet)
        }
    }

    // MARK: - checkPayment

    func test_checkPayment_succeeds_andForwardsJWS_whenWalletSucceeds() async {
        let wallet = MockWalletStoreKitInteractor(checkResult: .success(Date()))
        let interactor = makeInteractor(wallet: wallet)

        let result = await interactor.checkPayment(jwsRepresentation: "signed.jws.payload")

        if case let .failure(error) = result {
            XCTFail("Expected success, got failure: \(error)")
        }
        XCTAssertEqual(wallet.lastSubmittedJWS, "signed.jws.payload")
    }

    func test_checkPayment_propagatesWalletFailure() async {
        let wallet = MockWalletStoreKitInteractor(checkResult: .failure(.noWallet))
        let interactor = makeInteractor(wallet: wallet)

        let result = await interactor.checkPayment(jwsRepresentation: "x")

        switch result {
        case .success:
            XCTFail("Expected the wallet failure to propagate")
        case let .failure(error):
            XCTAssertEqual(error as? WarrenWalletInteractorError, .noWallet)
        }
    }
}
