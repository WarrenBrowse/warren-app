//
//  WarrenForumIdentityStoreTests.swift
//  WarrenVPNTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  Round-trip tests for the forum identity Keychain store. The Keychain is
//  real (no mock) so these run on the simulator, hosted by the app like the
//  wallet Keychain tests. Each test cleans up after itself.
//

import WarrenRustRuntime
import XCTest

@testable import WarrenVPN

final class WarrenForumIdentityStoreTests: XCTestCase {
    private let identity = WarrenForumIdentity(handle: "jugop-lobab-virar", notifySlot: 1)

    override func setUpWithError() throws {
        try super.setUpWithError()
        try? WarrenForumIdentityStore.delete()
        try? WarrenWalletKeychain.delete()
    }

    override func tearDownWithError() throws {
        try? WarrenForumIdentityStore.delete()
        try? WarrenWalletKeychain.delete()
        try super.tearDownWithError()
    }

    func testNothingIsStoredAtFirst() {
        XCTAssertNil(WarrenForumIdentityStore.load())
    }

    func testSaveThenLoadRoundTripsTheHandleAndTheSlot() throws {
        try WarrenForumIdentityStore.save(identity)
        XCTAssertEqual(WarrenForumIdentityStore.load(), identity)
    }

    func testAnIdentityWithoutASlotRoundTripsAsSuch() throws {
        // The slot is absent when the allocator had no room: it costs the
        // badge and nothing else, and must not come back as zero.
        let noSlot = WarrenForumIdentity(handle: "jugop-lobab-virar", notifySlot: nil)
        try WarrenForumIdentityStore.save(noSlot)
        XCTAssertEqual(WarrenForumIdentityStore.load(), noSlot)
    }

    func testASecondSaveReplacesTheFirst() throws {
        try WarrenForumIdentityStore.save(identity)
        let renewed = WarrenForumIdentity(handle: "lusab-babad-dovok", notifySlot: 7)
        try WarrenForumIdentityStore.save(renewed)
        XCTAssertEqual(WarrenForumIdentityStore.load(), renewed)
    }

    func testDeleteIsANoOpOnAnEmptyStore() {
        XCTAssertNoThrow(try WarrenForumIdentityStore.delete())
        XCTAssertNil(WarrenForumIdentityStore.load())
    }

    func testErasingTheWalletErasesTheForumIdentityWithIt() throws {
        // The handle is this wallet's pairwise name: a new wallet restored on
        // the same device must never be shown the previous one's.
        try WarrenWalletKeychain.save(
            mnemonic: "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about")
        try WarrenForumIdentityStore.save(identity)
        try WarrenWalletKeychain.delete()
        XCTAssertNil(WarrenForumIdentityStore.load())
    }

    func testASaveAnnouncesTheChange() throws {
        let announced = expectation(
            forNotification: WarrenForumIdentityStore.didChangeNotification, object: nil)
        try WarrenForumIdentityStore.save(identity)
        wait(for: [announced], timeout: 2)
    }
}
