//
//  ConnectionPhaseColorTests.swift
//  WarrenVPNTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import UIKit
import XCTest

@testable import WarrenVPN

/// The status card paints two different things with the phase colour, and they
/// are not the same colour. The eye, the rails and the buttons take the
/// saturated accent, where 3:1 against the surface is the bar. The title is
/// text at title size and needs 4.5:1, which the saturated accent misses on the
/// card's neutral plate, so it takes the lifted tint.
///
/// Desktop names the same split in `connection-phase.ts`
/// (`getPhaseAccentColorName` against `getPhaseTitleColorName`), and these are
/// its `redText` / `greenText` / `orangeText` values to the byte.
final class ConnectionPhaseColorTests: XCTestCase {
    private func components(_ color: UIColor) -> [Int] {
        var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
        color.getRed(&r, green: &g, blue: &b, alpha: &a)
        return [Int((r * 255).rounded()), Int((g * 255).rounded()), Int((b * 255).rounded())]
    }

    func testTheTitleTakesTheLiftedTintOfItsPhase() {
        XCTAssertEqual(components(ConnectionPhase.protected.titleColor), [150, 196, 116])
        XCTAssertEqual(components(ConnectionPhase.connecting.titleColor), [240, 163, 96])
        XCTAssertEqual(components(ConnectionPhase.interrupted.titleColor), [240, 163, 96])
        XCTAssertEqual(components(ConnectionPhase.exposed.titleColor), [233, 142, 122])
    }

    /// The kill-switch state has no hue of its own: neutral is its signal, so
    /// both the fill and the title stay white and there is nothing to lift.
    func testTheBlockedPhaseStaysWhiteOnBothRoles() {
        XCTAssertEqual(components(ConnectionPhase.blocked.titleColor), [255, 255, 255])
        XCTAssertEqual(components(ConnectionPhase.blocked.accentColor), [255, 255, 255])
    }

    /// Repointing the title must not de-saturate the eye with it: they read the
    /// same view-model property today, which is the whole reason this is a
    /// pair of tests rather than one.
    func testTheAccentStaysSaturatedForTheIconography() {
        XCTAssertEqual(components(ConnectionPhase.protected.accentColor), [110, 162, 78])
        XCTAssertEqual(components(ConnectionPhase.connecting.accentColor), [224, 122, 40])
        XCTAssertEqual(components(ConnectionPhase.exposed.accentColor), [202, 76, 56])
    }

    func testTheTitleAndTheAccentAreNotTheSameColour() {
        for phase in [ConnectionPhase.protected, .connecting, .interrupted, .exposed] {
            XCTAssertNotEqual(
                components(phase.titleColor),
                components(phase.accentColor),
                "\(phase) paints its title with the saturated fill"
            )
        }
    }

    /// Which token each phase takes comes from scenery.json, the table desktop
    /// and the browser extension read too; `SceneryTone.color` is the one place
    /// a token meets this palette.
    func testEachPhaseTakesTheTonesOfTheSharedTable() {
        XCTAssertEqual(ConnectionPhase.exposed.accentTone, .red)
        XCTAssertEqual(ConnectionPhase.connecting.accentTone, .orange)
        XCTAssertEqual(ConnectionPhase.protected.accentTone, .green)
        XCTAssertEqual(ConnectionPhase.interrupted.accentTone, .orange)
        XCTAssertEqual(ConnectionPhase.blocked.accentTone, .white)
        XCTAssertEqual(ConnectionPhase.exposed.titleTone, .redText)
        XCTAssertEqual(ConnectionPhase.protected.titleTone, .greenText)
        XCTAssertEqual(ConnectionPhase.blocked.titleTone, .white)
    }
}
