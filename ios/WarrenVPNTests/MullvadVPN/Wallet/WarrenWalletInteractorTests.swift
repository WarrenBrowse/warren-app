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

    func test_publicKeyShort_returnsNil_whenNoWallet() {
        let interactor = WarrenWalletInteractor()
        XCTAssertNil(interactor.publicKeyShort())
    }

    func test_publicKeyShort_formatsAsFirst4DotsLast4_whenWalletExists() throws {
        let wallet = try WarrenWallet.generate()
        try WarrenWalletKeychain.save(mnemonic: wallet.revealMnemonic())
        let fullHex = wallet.publicKeyHex
        wallet.forgetSecret()

        let interactor = WarrenWalletInteractor()
        let short = interactor.publicKeyShort()
        XCTAssertNotNil(short)
        XCTAssertEqual(short, "\(fullHex.prefix(4))...\(fullHex.suffix(4))")
        // Total length : 4 + 3 + 4 = 11
        XCTAssertEqual(short?.count, 11)
    }
}
