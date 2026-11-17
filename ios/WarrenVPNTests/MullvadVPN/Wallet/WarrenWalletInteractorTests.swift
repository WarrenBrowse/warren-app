//
//  WarrenWalletInteractorTests.swift
//  WarrenVPNTests
//
//  Created by Warren on 2026-05-22.
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  Tests for `WarrenWalletInteractor` helpers that don't require
//  biometric authentication. Covers `publicKeyHex()` sync path
//  (Keychain → mnemonic → pubkey hex round-trip).
//

import XCTest
@testable import WarrenVPN

@testable import WarrenRustRuntime

@MainActor
final class WarrenWalletInteractorTests: XCTestCase {
    override func setUp() async throws {
        try await super.setUp()
        // Clean Keychain state so each test starts isolated.
        try? WarrenWalletKeychain.delete()
    }

    override func tearDown() async throws {
        try? WarrenWalletKeychain.delete()
        try await super.tearDown()
    }

    func test_publicKeyHex_returnsNil_whenNoWalletPersisted() {
        let interactor = WarrenWalletInteractor()
        XCTAssertNil(interactor.publicKeyHex(), "Expected nil when no wallet is in Keychain")
    }

    func test_publicKeyHex_returns64CharLowerHex_whenWalletExists() throws {
        // Persist a wallet first.
        let wallet = try WarrenWallet.generate()
        try WarrenWalletKeychain.save(mnemonic: wallet.revealMnemonic())
        let expected = wallet.publicKeyHex
        wallet.forgetSecret()

        let interactor = WarrenWalletInteractor()
        let hex = interactor.publicKeyHex()
        XCTAssertEqual(hex, expected, "publicKeyHex should match the wallet's pubkey")
        XCTAssertEqual(hex?.count, 64)
    }

    func test_publicKeyHex_isStableAcrossMultipleCalls() throws {
        let wallet = try WarrenWallet.generate()
        try WarrenWalletKeychain.save(mnemonic: wallet.revealMnemonic())
        wallet.forgetSecret()

        let interactor = WarrenWalletInteractor()
        let first = interactor.publicKeyHex()
        let second = interactor.publicKeyHex()
        let third = interactor.publicKeyHex()
        XCTAssertNotNil(first)
        XCTAssertEqual(first, second)
        XCTAssertEqual(second, third)
    }

    func test_publicKeyAddress_returnsNil_whenNoWalletPersisted() {
        let interactor = WarrenWalletInteractor()
        XCTAssertNil(interactor.publicKeyAddress(), "Expected nil when no wallet is in Keychain")
    }

    func test_publicKeyAddress_returnsWarrenSS58Address_whenWalletExists() throws {
        let wallet = try WarrenWallet.generate()
        try WarrenWalletKeychain.save(mnemonic: wallet.revealMnemonic())
        let expected = wallet.publicKeyAddress
        wallet.forgetSecret()

        let interactor = WarrenWalletInteractor()
        let address = interactor.publicKeyAddress()
        XCTAssertEqual(address, expected, "publicKeyAddress should match the wallet's SS58 address")
        XCTAssertEqual(address?.isWarrenAddress, true)
    }

    func test_publicKeyShort_returnsNil_whenNoWallet() {
        let interactor = WarrenWalletInteractor()
        XCTAssertNil(interactor.publicKeyShort())
    }

    func test_publicKeyShort_formatsAsFirst6EllipsisLast6_whenWalletExists() throws {
        let wallet = try WarrenWallet.generate()
        try WarrenWalletKeychain.save(mnemonic: wallet.revealMnemonic())
        let fullAddress = wallet.publicKeyAddress
        wallet.forgetSecret()

        let interactor = WarrenWalletInteractor()
        let short = interactor.publicKeyShort()
        XCTAssertNotNil(short)
        XCTAssertEqual(short, "\(fullAddress.prefix(6))\u{2026}\(fullAddress.suffix(6))")
        // Total length : 6 + 1 (ellipsis) + 6 = 13
        XCTAssertEqual(short?.count, 13)
    }
}

/// Deleting the account asks for the end of the wallet address, which the
/// account "number" holds: an SS58 address in base58, letters included, and
/// not always a multiple of four characters long.
@MainActor
final class AccountDeletionViewModelTests: XCTestCase {
    /// Real addresses of both lengths the check used to trip on: one whose
    /// last four characters hold a letter, one whose length leaves a
    /// partial group of four at the end.
    private func realAddresses() throws -> [String] {
        try [
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
            "legal winner thank year wave sausage worth useful legal winner thank yellow",
            "letter advice cage absurd amount doctor acoustic avoid letter advice cage above",
            "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong",
        ].map { phrase in
            let wallet = try WarrenWallet.fromMnemonic(phrase)
            defer { wallet.forgetSecret() }
            return wallet.publicKeyAddress
        }
    }

    func test_theLastFourCharactersOfTheAddressUnlockDeletion() throws {
        for address in try realAddresses() {
            let viewModel = AccountDeletionViewModel(
                backEnd: MockAccountDeletionBackEnd(accountNumber: address)
            )
            viewModel.enteredAccountNumberSuffix = String(address.suffix(4))

            XCTAssertTrue(viewModel.canDelete, "address of \(address.count) characters")
            XCTAssertEqual(try viewModel.validate(input: String(address.suffix(4))).get(), address)
        }
    }

    func test_otherCharactersDoNotUnlockDeletion() throws {
        let address = try XCTUnwrap(realAddresses().first)
        let viewModel = AccountDeletionViewModel(backEnd: MockAccountDeletionBackEnd(accountNumber: address))
        let wrong = String(address.suffix(5).prefix(4))
        viewModel.enteredAccountNumberSuffix = wrong

        XCTAssertFalse(viewModel.canDelete)
        XCTAssertThrowsError(try viewModel.validate(input: wrong).get())
    }
}
