//
//  HeaderBarViewTests.swift
//  WarrenVPNTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import UIKit
import XCTest

@testable import WarrenRustRuntime
@testable import WarrenVPN

/// The header is the only place inside the app that can say which product this
/// install is: the home screen icon is out of sight once the app is open, and
/// the status-bar VPN chip is drawn by the system with no app-supplied content.
final class HeaderBarViewTests: XCTestCase {
    func testTheChipCarriesTheMarkerOfTheCompiledEnvironment() {
        let header = HeaderBarView(frame: .zero)

        XCTAssertEqual(header.productChipLabel.text, WarrenProductAnchors.current.environmentBadge)
        XCTAssertEqual(header.productChipLabel.isHidden, WarrenProductAnchors.current.isProd)
    }

    func testAMarkedBuildShowsTheChipAndProdShowsNothing() {
        let header = HeaderBarView(frame: .zero)

        header.productBadge = "BETA"
        XCTAssertEqual(header.productChipLabel.text, "BETA")
        XCTAssertFalse(header.productChipLabel.isHidden)

        header.productBadge = nil
        XCTAssertNil(header.productChipLabel.text)
        XCTAssertTrue(header.productChipLabel.isHidden)
    }

    /// `applyTone()` re-tints the whole header when it moves from the charcoal
    /// bar to the bright scenery sky. A chip left out of it keeps one tone's
    /// edge colour and dissolves into the other backdrop, which is exactly the
    /// connect screen.
    func testTheChipIsRetintedWithTheRestOfTheHeader() throws {
        let header = HeaderBarView(frame: .zero)
        header.productBadge = "BETA"

        header.tone = .light
        let overCharcoal = try XCTUnwrap(header.productChipLabel.layer.borderColor)
        header.tone = .dark
        let overScenery = try XCTUnwrap(header.productChipLabel.layer.borderColor)

        XCTAssertNotEqual(overCharcoal, overScenery)
        // The fill stays the brand amber in both tones: it is the same signal
        // the badged app icon and the desktop tray pip carry.
        XCTAssertEqual(header.productChipLabel.backgroundColor, UIColor.Warren.yellow)
    }
}
