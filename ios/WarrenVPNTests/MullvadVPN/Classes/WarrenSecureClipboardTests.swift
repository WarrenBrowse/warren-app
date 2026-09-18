//
//  WarrenSecureClipboardTests.swift
//  WarrenVPNTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import UIKit
import XCTest

@testable import WarrenVPN

/// A secret on the clipboard must not leave the device. A plain
/// `UIPasteboard.general.string = secret` publishes it to Universal Clipboard,
/// which is how the recovery phrase would reach every Mac signed into the same
/// Apple account.
final class WarrenSecureClipboardTests: XCTestCase {
    private var pasteboard: UIPasteboard!

    override func setUpWithError() throws {
        try super.setUpWithError()
        pasteboard = UIPasteboard.withUniqueName()
    }

    override func tearDownWithError() throws {
        UIPasteboard.remove(withName: pasteboard.name)
        pasteboard = nil
        try super.tearDownWithError()
    }

    func testACopiedSecretIsThereToPaste() {
        WarrenSecureClipboard.copy("dawn hint fabric ...", to: pasteboard)
        XCTAssertEqual(pasteboard.string, "dawn hint fabric ...")
        XCTAssertEqual(pasteboard.items.count, 1)
    }

    /// The expiry is enforced by the system, so it holds even when the app is
    /// killed before its own timer fires. `localOnly` has no readable
    /// counterpart and no local effect, so nothing here can observe it; what
    /// keeps it in place is `copy` being the only way this app writes a secret.
    func testTheSystemTakesTheSecretBackWhenItExpires() {
        WarrenSecureClipboard.copy("dawn hint fabric ...", expiresIn: 1, to: pasteboard)
        XCTAssertEqual(pasteboard.string, "dawn hint fabric ...")

        let expired = expectation(description: "the clipboard expires the secret")
        DispatchQueue.global().asyncAfter(deadline: .now() + 2.5) { expired.fulfill() }
        wait(for: [expired], timeout: 5)

        XCTAssertTrue(pasteboard.items.isEmpty, "the secret outlived its expiry")
    }

    func testClearingTakesBackASecretNobodyPasted() {
        WarrenSecureClipboard.copy("dawn hint fabric ...", to: pasteboard)
        WarrenSecureClipboard.clearIfStillHolding("dawn hint fabric ...", in: pasteboard)
        XCTAssertTrue(pasteboard.items.isEmpty)
    }

    func testClearingLeavesWhateverTheUserCopiedSince() {
        WarrenSecureClipboard.copy("dawn hint fabric ...", to: pasteboard)
        pasteboard.string = "a shopping list"
        WarrenSecureClipboard.clearIfStillHolding("dawn hint fabric ...", in: pasteboard)
        XCTAssertEqual(pasteboard.string, "a shopping list")
    }
}
