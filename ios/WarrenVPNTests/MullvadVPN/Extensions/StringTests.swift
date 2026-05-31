//
//  StringTests.swift
//  MullvadVPNTests
//
//  Created by pronebird on 27/03/2020.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import XCTest

class StringTests: XCTestCase {
    /// A valid Warren SS58 address (48 chars, `wb` prefix) used across
    /// the address-helper tests below.
    private let sampleAddress = "wb7kgy8FF4rx4tamkksPfoymeeeZVXLrnSjbBxCun3XhP9DnB"

    func testEmptyString() {
        XCTAssertTrue("".split(every: 4).isEmpty)
    }

    func testString() {
        XCTAssertEqual("12345678".split(every: 4), ["1234", "5678"])
    }

    func testOddString() {
        XCTAssertEqual("123456789".split(every: 4), ["1234", "5678", "9"])
    }

    func testStringShorterThanLength() {
        XCTAssertEqual("1".split(every: 4), ["1"])
    }

    // MARK: - Warren wallet address helpers

    func testShortWarrenAddress_abbreviatesToFirst6EllipsisLast6() {
        // Expected: first 6 ("wb7kgy") + … + last 6 ("hP9DnB").
        XCTAssertEqual(sampleAddress.shortWarrenAddress, "wb7kgy\u{2026}hP9DnB")
        XCTAssertEqual(sampleAddress.shortWarrenAddress.count, 13)
    }

    func testShortWarrenAddress_returnsUnchanged_whenShortEnough() {
        // 13 chars or fewer must be returned verbatim (no ellipsis).
        XCTAssertEqual("wb7kgy8FF4rx4".shortWarrenAddress, "wb7kgy8FF4rx4")
        XCTAssertEqual("short".shortWarrenAddress, "short")
        XCTAssertEqual("".shortWarrenAddress, "")
    }

    func testIsWarrenAddress_acceptsValidAddress() {
        XCTAssertTrue(sampleAddress.isWarrenAddress)
    }

    func testIsWarrenAddress_rejectsWrongPrefix() {
        // Same length/charset but no `wb` prefix.
        XCTAssertFalse("xy7kgy8FF4rx4tamkksPfoymeeeZVXLrnSjbBxCun3XhP9DnB".isWarrenAddress)
    }

    func testIsWarrenAddress_rejectsWrongLength() {
        XCTAssertFalse("wb".isWarrenAddress) // too short
        XCTAssertFalse("wb7kgy".isWarrenAddress) // too short
        // 50 chars (one over the 49-char upper bound).
        XCTAssertFalse("wb7kgy8FF4rx4tamkksPfoymeeeZVXLrnSjbBxCun3XhP9DnBx".isWarrenAddress)
    }

    func testIsWarrenAddress_rejectsNonBase58Characters() {
        // Contains `0`, `O`, `I`, `l` which are excluded from base58.
        XCTAssertFalse("wb0OlI8FF4rx4tamkksPfoymeeeZVXLrnSjbBxCun3XhP9DnB".isWarrenAddress)
    }

    func testIsWarrenAddress_rejectsEmpty() {
        XCTAssertFalse("".isWarrenAddress)
    }
}
