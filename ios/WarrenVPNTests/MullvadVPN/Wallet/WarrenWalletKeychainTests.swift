//
//  WarrenWalletKeychainTests.swift
//  WarrenVPNTests
//
//  Created by Warren on 2026-05-21.
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  Round-trip tests for the Warren wallet Keychain wrapper. The
//  Keychain is real (no mock) so these tests run only on simulator /
//  device, not on host CI. Each test cleans up after itself to avoid
//  cross-test contamination.
//

import XCTest
@testable import WarrenVPN

final class WarrenWalletKeychainTests: XCTestCase {
    override func setUpWithError() throws {
        try super.setUpWithError()
        // Clean up any leftover entry from prior runs.
        try? WarrenWalletKeychain.delete()
    }

    override func tearDownWithError() throws {
        try? WarrenWalletKeychain.delete()
        try super.tearDownWithError()
    }

    /// A freshly initialised Keychain has no wallet entry.
    func test_exists_returnsFalse_whenNoWalletStored() {
        XCTAssertFalse(WarrenWalletKeychain.exists())
    }

    /// `save` then `load` returns the same UTF-8 string.
    func test_saveAndLoad_roundtripsMnemonic() throws {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
        try WarrenWalletKeychain.save(mnemonic: mnemonic)
        XCTAssertTrue(WarrenWalletKeychain.exists())
        let loaded = try WarrenWalletKeychain.load()
        XCTAssertEqual(loaded, mnemonic)
    }

    /// Calling `save` twice overwrites the existing entry rather than
    /// erroring out. Required for the "retry generate" UX path.
    func test_save_isIdempotent_overwritesExistingEntry() throws {
        let first = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
        let second = "legal winner thank year wave sausage worth useful legal winner thank yellow"
        try WarrenWalletKeychain.save(mnemonic: first)
        try WarrenWalletKeychain.save(mnemonic: second)
        let loaded = try WarrenWalletKeychain.load()
        XCTAssertEqual(loaded, second)
    }

    /// `load` on an empty Keychain throws `.notFound`.
    func test_load_throws_whenNoWalletStored() {
        XCTAssertThrowsError(try WarrenWalletKeychain.load()) { error in
            XCTAssertEqual(error as? WarrenWalletKeychainError, .notFound)
        }
    }

    /// `delete` on an empty Keychain is a no-op (no throw).
    func test_delete_isIdempotent_onEmptyKeychain() {
        XCTAssertNoThrow(try WarrenWalletKeychain.delete())
        XCTAssertFalse(WarrenWalletKeychain.exists())
    }

    /// `delete` followed by `load` throws `.notFound`.
    func test_deleteThenLoad_throwsNotFound() throws {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
        try WarrenWalletKeychain.save(mnemonic: mnemonic)
        try WarrenWalletKeychain.delete()
        XCTAssertFalse(WarrenWalletKeychain.exists())
        XCTAssertThrowsError(try WarrenWalletKeychain.load()) { error in
            XCTAssertEqual(error as? WarrenWalletKeychainError, .notFound)
        }
    }

    /// Non-ASCII mnemonic strings round-trip safely (BIP39 in non-
    /// English languages is supported by warren-identity).
    func test_saveAndLoad_handlesNonAsciiMnemonic() throws {
        let frenchMnemonic = "abeille abeille abeille abeille abeille abeille abeille abeille abeille abeille abeille abeille"
        try WarrenWalletKeychain.save(mnemonic: frenchMnemonic)
        let loaded = try WarrenWalletKeychain.load()
        XCTAssertEqual(loaded, frenchMnemonic)
    }
}
