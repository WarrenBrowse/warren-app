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
        ("hero.logoHeight", M.Hero.logoHeight),
        ("hero.logoInset", M.Hero.logoInset),
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
        ("toolbar.badgeInset", M.Toolbar.badgeInset),
        ("toolbar.badgeRing", M.Toolbar.badgeRing),
        ("toolbar.blur", M.Toolbar.blur),
        ("notice.marginTop", M.Notice.marginTop),
        ("notice.maxWidth", M.Notice.maxWidth),
        ("notice.radius", M.Notice.radius),
        ("notice.blur", M.Notice.blur),
        ("notice.gap", M.Notice.gap),
        ("notice.paddingVertical", M.Notice.paddingVertical),
        ("notice.paddingStart", M.Notice.paddingStart),
        ("notice.paddingEnd", M.Notice.paddingEnd),
        ("notice.buttonInset", M.Notice.buttonInset),
        ("notice.icon", M.Notice.icon),
        ("notice.close", M.Notice.close),
        ("notice.titleSize", M.Notice.titleSize),
        ("notice.titleWeight", M.Notice.titleWeight),
        ("notice.bodySize", M.Notice.bodySize),
        ("notice.bodyGap", M.Notice.bodyGap),
        ("notice.buttonHeight", M.Notice.buttonHeight),
        ("notice.buttonPaddingHorizontal", M.Notice.buttonPaddingHorizontal),
        ("notice.buttonRadius", M.Notice.buttonRadius),
        ("notice.buttonTextSize", M.Notice.buttonTextSize),
        ("notice.buttonTextWeight", M.Notice.buttonTextWeight),
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
        ("subtitle.marginTop", M.Subtitle.marginTop),
        ("subtitle.paddingVertical", M.Subtitle.paddingVertical),
        ("subtitle.lineHeight", M.Subtitle.lineHeight),
        ("subtitle.textSize", M.Subtitle.textSize),
        ("subtitle.monoSize", M.Subtitle.monoSize),
        ("exitRow.marginTop", M.ExitRow.marginTop),
        ("exitRow.outset", M.ExitRow.outset),
        ("exitRow.paddingVertical", M.ExitRow.paddingVertical),
        ("exitRow.radius", M.ExitRow.radius),
        ("exitRow.gap", M.ExitRow.gap),
        ("exitRow.textSize", M.ExitRow.textSize),
        ("exitRow.ring", M.ExitRow.ring),
        ("exitRow.ringHole", M.ExitRow.ringHole),
        ("exitRow.person", M.ExitRow.person),
        ("exitRow.partGap", M.ExitRow.partGap),
        ("exitRow.chevron", M.ExitRow.chevron),
        ("exitRow.chevronMs", M.ExitRow.chevronMs),
        ("details.marginTop", M.Details.marginTop),
        ("details.radius", M.Details.radius),
        ("details.paddingVertical", M.Details.paddingVertical),
        ("details.paddingHorizontal", M.Details.paddingHorizontal),
        ("details.columnGap", M.Details.columnGap),
        ("details.rowGap", M.Details.rowGap),
        ("details.textSize", M.Details.textSize),
        ("details.valueWeight", M.Details.valueWeight),
        ("details.monoSize", M.Details.monoSize),
        ("details.dividerMargin", M.Details.dividerMargin),
        ("details.chipSize", M.Details.chipSize),
        ("details.chipWeight", M.Details.chipWeight),
        ("details.chipPaddingVertical", M.Details.chipPaddingVertical),
        ("details.chipPaddingHorizontal", M.Details.chipPaddingHorizontal),
        ("controls.marginTop", M.Controls.marginTop),
        ("controls.gap", M.Controls.gap),
        ("controls.rowHeight", M.Controls.rowHeight),
        ("controls.radius", M.Controls.radius),
        ("controls.thumb", M.Controls.thumb),
        ("controls.thumbLine", M.Controls.thumbLine),
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
        let mirrored = Set(Self.expected.map(\.0)).union(Self.retired)
        XCTAssertEqual(shared.subtracting(mirrored), [], "home tokens with no iOS copy")
    }

    /// Tokens the home does not read. None today; one that leaves the home is named here rather
    /// than dropped from the table silently.
    private static let retired: Set<String> = []

    /// The panel column is the screen up to the tablet cap, and it is what sizes the hero.
    func testTheColumnIsTheScreenWidthUpToTheTabletCap() {
        XCTAssertEqual(HomeMetrics.columnWidth(forWidth: 393), 393)
        XCTAssertEqual(HomeMetrics.columnWidth(forWidth: 1032), HomeMetrics.maxColumnWidth)
    }

    /// The art is clear of the toolbar below its bottom edge plus the margin, under the status bar.
    func testTheClearTopIsTheToolbarBottomPlusTheMargin() {
        XCTAssertEqual(HomeMetrics.clearTop(safeTop: 0), 68)
        XCTAssertEqual(HomeMetrics.clearTop(safeTop: 62), 130)
    }

    // MARK: - Sharing the screen

    /// A 400 point phone under a 50 point status bar: the hero is 50 + 400 * 300 / 380 at least,
    /// 50 + 400 * 1.15 at most, 50 + 400 * 176 / 380 while compact. The panel is 300 high with its
    /// 18 point overlap of the hero, the footer 44.
    private func phone(height: CGFloat, detailsOpen: Bool = false) -> HomeLayout {
        HomeLayout.resolve(
            width: 400, height: height, safeTop: 50, panelHeight: 300, footerHeight: 44, detailsOpen: detailsOpen)
    }

    func testATallPhoneGrowsTheHeroToItsMaximumAndLeavesTheRestAboveTheFooter() {
        let layout = phone(height: 1000)
        XCTAssertEqual(layout.heroHeight, 510, accuracy: 0.01)
        XCTAssertEqual(layout.fullHeight, 510, accuracy: 0.01)
        XCTAssertEqual(layout.gap, 164, accuracy: 0.01)
        XCTAssertFalse(layout.compactFraming)
        XCTAssertFalse(layout.scrolls)
    }

    func testAPhoneBetweenTheBoundsGivesAllItsFreeHeightToTheHero() {
        let layout = phone(height: 800)
        XCTAssertEqual(layout.heroHeight, 474, accuracy: 0.01)
        XCTAssertEqual(layout.gap, 0)
        XCTAssertFalse(layout.compactFraming)
    }

    func testAShortScreenLetsTheHeroGoDownToCompactFramedTheCompactWay() {
        let layout = phone(height: 600)
        XCTAssertEqual(layout.heroHeight, 274, accuracy: 0.01)
        XCTAssertTrue(layout.compactFraming)
        XCTAssertFalse(layout.scrolls)
        // Framed at the scale of the minimum hero, so the art slides up instead of narrowing.
        XCTAssertEqual(layout.fullHeight, 50 + 400 * 300 / 380, accuracy: 0.01)
    }

    func testAScreenTooShortForTheCompactHeroScrollsThePanel() {
        let layout = phone(height: 500)
        XCTAssertEqual(layout.heroHeight, 50 + 400 * 176 / 380, accuracy: 0.01)
        XCTAssertTrue(layout.scrolls)
        XCTAssertEqual(layout.gap, 0)
    }

    func testTheOpenDetailsTakeTheCompactHero() {
        let layout = phone(height: 1000, detailsOpen: true)
        XCTAssertEqual(layout.heroHeight, 50 + 400 * 176 / 380, accuracy: 0.01)
        XCTAssertTrue(layout.compactFraming)
        XCTAssertFalse(layout.scrolls)
    }

    /// A tablet sizes its hero from the 480 point column, not from its full width.
    func testAWideScreenSizesTheHeroFromTheColumn() {
        let layout = HomeLayout.resolve(
            width: 1024, height: 1366, safeTop: 24, panelHeight: 300, footerHeight: 44, detailsOpen: false)
        XCTAssertEqual(layout.heroHeight, 24 + 480 * 1.15, accuracy: 0.01)
    }
}
