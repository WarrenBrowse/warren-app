//
//  HomeMetricsTests.swift
//  WarrenVPNTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import XCTest

@testable import WarrenVPN

/// The home geometry desktop generates into `design-tokens.json` `home`, held to its iOS copy:
/// a value changed on desktop and not carried over fails here instead of drifting on a phone.
final class HomeMetricsTests: XCTestCase {
    /// `ios/WarrenVPNTests/MullvadVPN/View controllers/Tunnel/HomeMetricsTests.swift`
    private static let repoDir = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .deletingLastPathComponent()

    private func homeSection() throws -> [String: [String: Any]] {
        let data = try Data(contentsOf: Self.repoDir.appendingPathComponent("design-tokens.json"))
        let tokens = try XCTUnwrap(try JSONSerialization.jsonObject(with: data) as? [String: Any])
        return try XCTUnwrap(tokens["home"] as? [String: [String: Any]], "no home section")
    }

    private typealias M = HomeMetrics

    private static let expected: [(String, CGFloat)] = [
        ("hero.headerInset", M.Hero.headerInset),
        ("hero.clearMargin", M.Hero.clearMargin),
        ("hero.fade", M.Hero.fade),
        ("hero.logoHeight", M.Hero.logoHeight),
        ("hero.scrimStop", M.Hero.scrimStop),
        ("hero.glowWidth", M.Hero.glowWidth),
        ("hero.glowHeight", M.Hero.glowHeight),
        ("hero.frameMs", M.Hero.frameMs),
        ("hero.crossfadeMs", M.Hero.crossfadeMs),
        ("hero.bulaMs", M.Hero.bulaMs),
        ("hero.blurRadius", M.Hero.blurRadius),
        ("toolbar.height", M.Toolbar.height),
        ("toolbar.radius", M.Toolbar.radius),
        ("toolbar.paddingHorizontal", M.Toolbar.paddingHorizontal),
        ("toolbar.button", M.Toolbar.button),
        ("toolbar.icon", M.Toolbar.icon),
        ("toolbar.badge", M.Toolbar.badge),
        ("toolbar.badgeRing", M.Toolbar.badgeRing),
        ("toolbar.blur", M.Toolbar.blur),
        ("panel.paddingHorizontal", M.Panel.paddingHorizontal),
        ("panel.overlap", M.Panel.overlap),
        ("panel.gap", M.Panel.gap),
        ("pill.paddingVertical", M.Pill.paddingVertical),
        ("pill.paddingStart", M.Pill.paddingStart),
        ("pill.paddingEnd", M.Pill.paddingEnd),
        ("pill.gap", M.Pill.gap),
        ("pill.icon", M.Pill.icon),
        ("pill.textSize", M.Pill.textSize),
        ("pill.textWeight", M.Pill.textWeight),
        ("title.marginTop", M.Title.marginTop),
        ("title.textSize", M.Title.textSize),
        ("title.textWeight", M.Title.textWeight),
        ("title.letterSpacingEm", M.Title.letterSpacingEm),
        ("subtitle.textSize", M.Subtitle.textSize),
        ("subtitle.monoSize", M.Subtitle.monoSize),
        ("controls.marginTop", M.Controls.marginTop),
        ("controls.gap", M.Controls.gap),
        ("controls.rowHeight", M.Controls.rowHeight),
        ("controls.radius", M.Controls.radius),
        ("controls.thumb", M.Controls.thumb),
        ("controls.paddingStart", M.Controls.paddingStart),
        ("controls.paddingEnd", M.Controls.paddingEnd),
        ("controls.rowGap", M.Controls.rowGap),
        ("controls.labelSize", M.Controls.labelSize),
        ("controls.labelWeight", M.Controls.labelWeight),
        ("controls.changeSize", M.Controls.changeSize),
        ("controls.buttonHeight", M.Controls.buttonHeight),
        ("controls.buttonTextSize", M.Controls.buttonTextSize),
        ("controls.buttonTextWeight", M.Controls.buttonTextWeight),
        ("controls.outlineWidth", M.Controls.outlineWidth),
        ("chips.marginTop", M.Chips.marginTop),
        ("chips.height", M.Chips.height),
        ("chips.gap", M.Chips.gap),
        ("chips.textSize", M.Chips.textSize),
        ("chips.paddingVertical", M.Chips.paddingVertical),
        ("chips.paddingHorizontal", M.Chips.paddingHorizontal),
        ("chips.borderWidth", M.Chips.borderWidth),
        ("footer.paddingVertical", M.Footer.paddingVertical),
        ("footer.paddingHorizontal", M.Footer.paddingHorizontal),
        ("footer.textSize", M.Footer.textSize),
        ("footer.icon", M.Footer.icon),
        ("footer.gap", M.Footer.gap),
    ]

    func testEveryHomeMeasurementMatchesTheSharedTable() throws {
        let home = try homeSection()
        for (path, value) in Self.expected {
            let parts = path.split(separator: ".").map(String.init)
            let entry = try XCTUnwrap(home[parts[0]]?[parts[1]] as? [String: Any], "\(path) left the table")
            let shared = try XCTUnwrap(entry["value"] as? Double, path)
            XCTAssertEqual(Double(value), shared, accuracy: 0.0001, path)
        }
    }

    /// Every entry of the shared table has its iOS copy: a token added on desktop and never carried
    /// over fails here rather than going unnoticed.
    func testNoSharedHomeMeasurementIsLeftWithoutACopy() throws {
        let home = try homeSection()
        let shared = Set(home.flatMap { section, entries in entries.keys.map { "\(section).\($0)" } })
        let mirrored = Set(Self.expected.map(\.0))
        XCTAssertEqual(shared.subtracting(mirrored), [], "home tokens with no iOS copy")
    }

    /// The art is clear of the toolbar below its bottom edge plus the margin, under the status bar.
    func testTheClearTopIsTheToolbarBottomPlusTheMargin() {
        XCTAssertEqual(HomeMetrics.clearTop(safeTop: 0), 68)
        XCTAssertEqual(HomeMetrics.clearTop(safeTop: 62), 130)
    }
}
