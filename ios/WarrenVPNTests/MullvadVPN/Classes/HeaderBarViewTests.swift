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
        // the badged app icon carries.
        XCTAssertEqual(header.productChipLabel.backgroundColor, UIColor.Warren.yellow)
    }

    // MARK: - The forum slot

    /// The slot carries the bell, the lifebuoy or nothing, and the badge only
    /// ever rides the bell: a count over the lifebuoy would promise activity
    /// to a wallet that has no forum account to have any.
    func testTheForumSlotShowsOnlyWhatThePairAllows() {
        let header = HeaderBarView(frame: .zero)
        header.forumUnread = 3

        header.forumSlot = .none
        XCTAssertTrue(header.forumButton.isHidden)
        XCTAssertTrue(header.forumBadgeLabel.isHidden)

        header.forumSlot = .community
        XCTAssertFalse(header.forumButton.isHidden)
        XCTAssertTrue(header.forumBadgeLabel.isHidden)

        header.forumSlot = .activity
        XCTAssertFalse(header.forumButton.isHidden)
        XCTAssertFalse(header.forumBadgeLabel.isHidden)
        XCTAssertEqual(header.forumBadgeLabel.text, "3")
    }

    /// An empty badge would invite a click into a panel with nothing in it.
    func testAnEmptyCountShowsNoBadge() {
        let header = HeaderBarView(frame: .zero)
        header.forumSlot = .activity

        header.forumUnread = 0
        XCTAssertTrue(header.forumBadgeLabel.isHidden)

        header.forumUnread = 1
        XCTAssertFalse(header.forumBadgeLabel.isHidden)
    }

    /// The badge saturates rather than growing, on the rule
    /// `fixtures/client-rules/forum_activity.json` pins for all three clients.
    func testTheBadgeSaturatesRatherThanGrowing() {
        let header = HeaderBarView(frame: .zero)
        header.forumSlot = .activity

        header.forumUnread = warrenUnreadSaturated
        XCTAssertEqual(header.forumBadgeLabel.text, "15+")
    }

    /// What is drawn and what VoiceOver reads come from the same two facts,
    /// so a count on screen is always a count spoken.
    func testTheSpokenLabelFollowsTheSlotAndTheCount() throws {
        let header = HeaderBarView(frame: .zero)

        header.forumSlot = .community
        let lifebuoy = try XCTUnwrap(header.forumButton.accessibilityLabel)

        header.forumSlot = .activity
        header.forumUnread = 0
        let quiet = try XCTUnwrap(header.forumButton.accessibilityLabel)

        header.forumUnread = 4
        let waiting = try XCTUnwrap(header.forumButton.accessibilityLabel)

        XCTAssertNotEqual(lifebuoy, quiet)
        XCTAssertNotEqual(quiet, waiting)
        XCTAssertTrue(waiting.contains("4"), waiting)
    }

    /// The forum glyph rides the same two backdrops as the rest of the header,
    /// so it is re-tinted with them rather than fixed at init.
    func testTheForumGlyphIsRetintedWithTheRestOfTheHeader() throws {
        let header = HeaderBarView(frame: .zero)
        header.forumSlot = .activity

        header.tone = .light
        let overCharcoal = try XCTUnwrap(header.forumButton.image(for: .normal))
        header.tone = .dark
        let overScenery = try XCTUnwrap(header.forumButton.image(for: .normal))

        XCTAssertNotEqual(overCharcoal, overScenery)
    }
}
