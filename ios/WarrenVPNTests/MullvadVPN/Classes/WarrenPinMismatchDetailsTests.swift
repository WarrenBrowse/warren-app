//
//  WarrenPinMismatchDetailsTests.swift
//  WarrenVPNTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import XCTest

@testable import WarrenVPN

/// The exit-key change alert asks the user to make a security decision, and it
/// shipped without the evidence to make it on: the two keys and the exit it
/// concerns were nowhere on the screen, and ten of the dialog's sixteen strings
/// had no iOS entry at all while Android carried all of them in 24 languages.
final class WarrenPinMismatchDetailsTests: XCTestCase {
    private let pinned = String(repeating: "a", count: 64)
    private let observed = String(repeating: "b", count: 64)

    func testAFingerprintIsShownHeadAndTail() {
        XCTAssertEqual(
            WarrenPinMismatchDetails.truncate(pinned),
            "aaaaaaaa...aaaaaaaa")
    }

    /// The desktop twin returns short input unchanged rather than padding it.
    func testAFingerprintShortEnoughToFitIsLeftAlone() {
        XCTAssertEqual(WarrenPinMismatchDetails.truncate("aabb", chars: 4), "aabb")
        XCTAssertEqual(WarrenPinMismatchDetails.truncate("aabbccdd", chars: 4), "aabbccdd")
        // One character past the fit is the first that truncates.
        XCTAssertEqual(WarrenPinMismatchDetails.truncate("aabbccdde", chars: 4), "aabb...cdde")
    }

    func testTheRowsCarryBothKeysAndTheExitTheyBelongTo() {
        let rows = WarrenPinMismatchDetails.rows(
            exitId: "0123456789abcdef0123", pinned: pinned, observed: observed, country: "ch")
        let labels = rows.map(\.label)
        XCTAssertEqual(labels, ["Exit ID", "Previously pinned key", "Newly observed key", "Location"])
        XCTAssertEqual(rows[1].value, "aaaaaaaa...aaaaaaaa")
        XCTAssertEqual(rows[2].value, "bbbbbbbb...bbbbbbbb")
        XCTAssertNotEqual(
            rows[1].value, rows[2].value,
            "the two keys render the same, so the user cannot see what changed")
    }

    /// An empty country reads as missing data rather than as unknown, so the
    /// row is left out entirely, as it is on desktop.
    func testTheLocationRowIsOmittedWhenTheExitReportedNoCountry() {
        for country in ["", "   "] {
            let rows = WarrenPinMismatchDetails.rows(
                exitId: "exit", pinned: pinned, observed: observed, country: country)
            XCTAssertFalse(rows.contains { $0.label == "Location" }, "country=\(country.debugDescription)")
        }
    }
}
