//
//  SceneryLayoutTests.swift
//  WarrenVPNTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import XCTest

@testable import WarrenVPN

/// The scenery placement replayed from `fixtures/client-rules/scenery_layout.json`, the file the
/// Android reader replays too. A geometry change means changing that file and every reader in the
/// same commit; a reader is never loosened to pass.
final class SceneryLayoutTests: XCTestCase {
    private static let tolerance: CGFloat = 0.02

    private func fixture() throws -> [String: Any] {
        try ClientRulesFixtures.load("scenery_layout.json")
    }

    private func number(_ object: [String: Any], _ key: String) throws -> CGFloat {
        guard let value = object[key] as? NSNumber else {
            throw ClientRulesFixtures.Failure.missingKey(key)
        }
        return CGFloat(value.doubleValue)
    }

    private func numbers(_ object: [String: Any], _ key: String) throws -> [CGFloat] {
        guard let value = object[key] as? [NSNumber] else {
            throw ClientRulesFixtures.Failure.missingKey(key)
        }
        return value.map { CGFloat($0.doubleValue) }
    }

    /// The fixture speaks in device pixels; iOS lays out in points, so a case is replayed at its
    /// point size and the expectations are divided by the same density.
    private func placement(for testCase: [String: Any]) throws -> (
        placement: SceneryLayout.Placement, density: CGFloat, bounds: CGRect, cardTop: CGFloat
    ) {
        let screen = try numbers(testCase, "screen_px")
        let density = try number(testCase, "density")
        let bounds = CGRect(
            x: 0, y: 0, width: screen[0] / density, height: screen[1] / density)
        let cardTop = try number(testCase, "card_top_dp")
        return (
            SceneryLayout.placement(in: bounds, cardTop: cardTop), density, bounds, cardTop
        )
    }

    func testTheCanvasAndTheRowsTheFormulaKeysOnAreTheOnesTheAssetsCarry() throws {
        let fixture = try fixture()
        let canvas = try numbers(fixture, "canvas_px")
        XCTAssertEqual(SceneryLayout.canvasWidth, canvas[0])
        XCTAssertEqual(SceneryLayout.canvasHeight, canvas[1])
        XCTAssertEqual(SceneryLayout.feetRow, try number(fixture, "feet_row"))
        XCTAssertEqual(SceneryLayout.groundRow, try number(fixture, "ground_row"))
        XCTAssertEqual(SceneryLayout.gap, try number(fixture, "gap_dp"))
        XCTAssertEqual(SceneryLayout.maxCanvasPan, try number(fixture, "max_canvas_pan_dp"))
    }

    func testEveryFixtureCasePlacesTheLayersWhereTheSharedFormulaSays() throws {
        let cases = try ClientRulesFixtures.cases(try fixture(), "cases")
        XCTAssertFalse(cases.isEmpty, "the fixture must carry cases")
        for testCase in cases {
            let name = try ClientRulesFixtures.string(testCase, "name")
            let (got, density, _, _) = try placement(for: testCase)
            let expect = try ClientRulesFixtures.object(testCase, "expect")

            func check(_ key: String, _ actual: CGFloat) throws {
                XCTAssertEqual(
                    try number(expect, key) / density, actual,
                    accuracy: Self.tolerance, "\(name).\(key)")
            }

            try check("canvas_height_px", got.canvasHeight)
            try check("canvas_pan_px", got.canvasPan)
            try check("foreground_shift_px", got.foregroundShift)
            try check("landscape_top_px", got.landscapeTop)
            try check("foreground_top_px", got.foregroundTop)
            try check("landscape_bottom_px", got.landscapeBottom)
            try check("foreground_bottom_px", got.foregroundBottom)
            try check("foreground_band_top_px", got.foregroundTop + got.groundOffset)
            try check("foreground_band_height_px", got.bandHeight)
            try check("foreground_band_left_px", got.bandLeft)
            try check("foreground_band_width_px", got.bandWidth)
            // The fixture's scale is pixels per canvas pixel; iOS lays out in points, so its own
            // scale is that divided by the density.
            XCTAssertEqual(
                try number(expect, "scale") / density, got.scale,
                accuracy: Self.tolerance, "\(name).scale")
        }
    }

    func testExactlyOneOfThePanAndTheSlideIsEverNonZero() throws {
        for testCase in try ClientRulesFixtures.cases(try fixture(), "cases") {
            let name = try ClientRulesFixtures.string(testCase, "name")
            let (got, _, _, _) = try placement(for: testCase)
            XCTAssertLessThanOrEqual(got.canvasPan, 0, "\(name): the canvas may only ever pan up")
            XCTAssertGreaterThanOrEqual(
                got.foregroundShift, 0, "\(name): the foreground may only ever slide down")
            XCTAssertTrue(
                got.canvasPan == 0 || got.foregroundShift == 0,
                "\(name): panning and sliding at once would move the foreground twice")
        }
    }

    func testTheStretchNeverReachesBulaOrTheBurrowMouth() throws {
        let fixture = try fixture()
        let maxStretch = try number(fixture, "max_band_stretch")
        for testCase in try ClientRulesFixtures.cases(fixture, "cases") {
            let name = try ClientRulesFixtures.string(testCase, "name")
            let (got, _, _, _) = try placement(for: testCase)
            XCTAssertGreaterThanOrEqual(
                got.groundOffset, SceneryLayout.feetRow * got.scale,
                "\(name): the split row would cut through Bula")
            let stretch = got.bandHeight / (got.canvasHeight - got.groundOffset)
            XCTAssertGreaterThanOrEqual(stretch, 1, "\(name): the band was compressed")
            XCTAssertLessThanOrEqual(stretch, maxStretch, "\(name): the band stretched \(stretch)")
        }
    }

    func testTheBandOverflowsTheScreenOnThreeSidesCarryingThePaperMarginOffIt() throws {
        for testCase in try ClientRulesFixtures.cases(try fixture(), "cases") {
            let name = try ClientRulesFixtures.string(testCase, "name")
            let (got, _, bounds, _) = try placement(for: testCase)
            XCTAssertLessThan(got.bandLeft, 0, "\(name): the band does not overhang the left edge")
            XCTAssertGreaterThan(
                got.bandLeft + got.bandWidth, bounds.width,
                "\(name): the band does not overhang the right edge")
            XCTAssertGreaterThanOrEqual(
                got.bandRect.maxY, bounds.height - Self.tolerance,
                "\(name): the band stops above the screen bottom")
        }
    }

    func testTheLandscapeIsDrawnWholeNeverStretched() throws {
        for testCase in try ClientRulesFixtures.cases(try fixture(), "cases") {
            let name = try ClientRulesFixtures.string(testCase, "name")
            let (got, _, _, _) = try placement(for: testCase)
            XCTAssertEqual(
                got.canvasHeight, got.landscapeBottom - got.landscapeTop,
                accuracy: Self.tolerance, "\(name): the landscape was scaled")
            XCTAssertLessThanOrEqual(
                got.foregroundTop + got.groundOffset, got.landscapeBottom + Self.tolerance,
                "\(name): a window opened between the landscape and the opaque ground")
        }
    }

    func testTheForegroundAlwaysReachesTheScreenBottom() throws {
        for testCase in try ClientRulesFixtures.cases(try fixture(), "cases") {
            let name = try ClientRulesFixtures.string(testCase, "name")
            let (got, _, bounds, _) = try placement(for: testCase)
            XCTAssertGreaterThanOrEqual(
                got.foregroundBottom, bounds.height - Self.tolerance,
                "\(name): the foreground stops above the screen bottom")
        }
    }

    func testABackdropLaidOutBeforeTheCardLeavesTheCanvasWhereItIsPainted() {
        let got = SceneryLayout.placement(
            in: CGRect(x: 0, y: 0, width: 393, height: 852), cardTop: nil)
        XCTAssertEqual(got.canvasPan, 0)
        XCTAssertEqual(got.foregroundShift, 0)
    }

    func testTheConnectingAnimationCarriesTheValuesTheOtherClientsUse() throws {
        let fixture = try self.fixture()
        XCTAssertEqual(SceneryLayout.connectingBlur, try number(fixture, "connecting_blur_dp"))
        XCTAssertEqual(SceneryLayout.connectingZoom, try number(fixture, "connecting_zoom"))
        XCTAssertEqual(SceneryLayout.connectingDim, try number(fixture, "connecting_dim"))
    }

    /// iOS blurs the source canvas, the other two blur the rendered view, so the
    /// radius only reads the same at the width it is computed for. Pinned across
    /// the whole range of iOS widths because a single constant stood here and was
    /// off by 0.60 to 1.9 times depending on the device.
    func testTheBlurIsTheSameStrengthOnScreenAtEveryWidth() throws {
        let displayed = try number(try fixture(), "connecting_blur_dp")
        for width in [CGFloat(320), 375, 393, 430, 744, 1024] {
            let canvasRadius = SceneryLayout.blurRadius(forWidth: width)
            // Carried back to display size, every width must give the fixture's
            // radius again.
            let onScreen = canvasRadius * width / SceneryLayout.canvasWidth
            XCTAssertEqual(
                onScreen, displayed, accuracy: 0.001,
                "a \(width) pt screen blurs at \(onScreen) where the other clients blur at \(displayed)")
        }
    }

    func testAZeroWidthCannotDivideTheBlurByNothing() {
        XCTAssertEqual(SceneryLayout.blurRadius(forWidth: 0), SceneryLayout.connectingBlur)
    }
}
