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

import Security
import WarrenRustRuntime
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

    /// Saving the phrase also stores the seed the packet tunnel extension
    /// reads: in the App Group's access group, which the extension's own App
    /// ID can reach, and readable after the first unlock, so an on-demand
    /// start on a locked device still finds it.
    func test_save_storesTunnelSeedInTheSharedGroupReadableAfterFirstUnlock() throws {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
        try WarrenWalletKeychain.save(mnemonic: mnemonic)

        let group = ApplicationConfiguration.securityGroupIdentifier
        let seed = try WarrenTunnelSeedKeychain.load(accessGroup: group)
        let wallet = try WarrenWallet.fromMnemonic(mnemonic)
        defer { wallet.forgetSecret() }
        XCTAssertEqual(seed, wallet.seed)

        let attributes = try XCTUnwrap(Self.tunnelSeedAttributes(accessGroup: group))
        XCTAssertEqual(attributes[kSecAttrAccessGroup as String] as? String, group)
        XCTAssertEqual(
            attributes[kSecAttrAccessible as String] as? String,
            kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly as String
        )
    }

    /// Erasing the wallet takes the extension's copy of the seed with it.
    func test_delete_removesTheTunnelSeed() throws {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
        try WarrenWalletKeychain.save(mnemonic: mnemonic)
        try WarrenWalletKeychain.delete()

        let group = ApplicationConfiguration.securityGroupIdentifier
        XCTAssertThrowsError(try WarrenTunnelSeedKeychain.load(accessGroup: group)) { error in
            XCTAssertEqual(error as? WarrenWalletKeychainError, .notFound)
        }
    }

    /// A wallet saved before the extension read its own item gets one at the
    /// next launch, without the user doing anything.
    func test_provisionTunnelSeed_restoresAMissingSeedFromThePhrase() throws {
        let mnemonic = "legal winner thank year wave sausage worth useful legal winner thank yellow"
        try WarrenWalletKeychain.save(mnemonic: mnemonic)
        let group = ApplicationConfiguration.securityGroupIdentifier
        try WarrenTunnelSeedKeychain.delete(accessGroup: group)

        try WarrenWalletKeychain.provisionTunnelSeedIfMissing()

        let wallet = try WarrenWallet.fromMnemonic(mnemonic)
        defer { wallet.forgetSecret() }
        XCTAssertEqual(try WarrenTunnelSeedKeychain.load(accessGroup: group), wallet.seed)
    }

    /// Without a wallet there is nothing to provision, and no error either:
    /// the launch path calls this on every start.
    func test_provisionTunnelSeed_withoutWallet_storesNothing() throws {
        try WarrenWalletKeychain.provisionTunnelSeedIfMissing()

        let group = ApplicationConfiguration.securityGroupIdentifier
        XCTAssertThrowsError(try WarrenTunnelSeedKeychain.load(accessGroup: group))
    }

    private static func tunnelSeedAttributes(accessGroup: String) -> [String: Any]? {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: WarrenWalletKeychain.service,
            kSecAttrAccount as String: WarrenTunnelSeedKeychain.account,
            kSecAttrAccessGroup as String: accessGroup,
            kSecReturnAttributes as String: kCFBooleanTrue!,
            kSecMatchLimit as String: kSecMatchLimitOne,
        ]
        var item: CFTypeRef?
        guard SecItemCopyMatching(query as CFDictionary, &item) == errSecSuccess else { return nil }
        return item as? [String: Any]
    }

    /// A phrase that derives no wallet is refused before anything is written,
    /// so the phrase item and the tunnel's seed item never disagree.
    func test_save_refusesAPhraseThatIsNotAWallet_andStoresNothing() {
        let notAWallet = "abeille abeille abeille abeille abeille abeille abeille abeille abeille abeille abeille abeille"
        XCTAssertThrowsError(try WarrenWalletKeychain.save(mnemonic: notAWallet))

        XCTAssertFalse(WarrenWalletKeychain.exists())
        let group = ApplicationConfiguration.securityGroupIdentifier
        XCTAssertThrowsError(try WarrenTunnelSeedKeychain.load(accessGroup: group))
    }
}
