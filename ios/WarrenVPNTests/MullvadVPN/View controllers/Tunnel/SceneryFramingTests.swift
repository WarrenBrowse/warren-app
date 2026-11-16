//
//  SceneryFramingTests.swift
//  WarrenVPNTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import XCTest

@testable import WarrenVPN

/// The hero framing replayed from `fixtures/client-rules/scenery_framing.json`, the file the
/// desktop reader replays too. A framing change means changing that file and every reader in the
/// same commit; a reader is never loosened to pass.
final class SceneryFramingTests: XCTestCase {
    /// The fixture rounds to two decimals; desktop checks to one.
    private static let tolerance: CGFloat = 0.06

    private func fixture() throws -> [String: Any] {
        try ClientRulesFixtures.load("scenery_framing.json")
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

    /// The fixture's px are the hero's own units; iOS replays them as points.
    private func frame(for testCase: [String: Any]) throws -> (SceneryFraming.Frame, SceneryFraming.Span) {
        let hero = try numbers(testCase, "hero_px")
        let rows = try numbers(testCase, "span_rows")
        let span = SceneryFraming.Span(top: rows[0], bottom: rows[1])
        let frame = SceneryFraming.frame(
            width: hero[0],
            height: hero[1],
            clearTop: try number(testCase, "clear_top_px"),
            fade: try number(testCase, "fade_px"),
            span: span
        )
        return (frame, span)
    }

    func testTheCanvasAndTheFeatherAreTheOnesTheFixtureDeclares() throws {
        let fixture = try fixture()
        let canvas = try numbers(fixture, "canvas_px")
        XCTAssertEqual(SceneryFraming.canvasWidth, canvas[0])
        XCTAssertEqual(SceneryFraming.canvasHeight, canvas[1])
        XCTAssertEqual(SceneryFraming.featherRows, try number(fixture, "feather_rows"))
    }

    func testTheSpansAreTheFixtureFramings() throws {
        let framings = try ClientRulesFixtures.object(try fixture(), "framings")
        let ground = try ClientRulesFixtures.object(framings, "ground")
        let sky = try ClientRulesFixtures.object(framings, "sky")
        XCTAssertEqual(SceneryFraming.groundSpan.top, try number(ground, "top_row"))
        XCTAssertEqual(SceneryFraming.groundSpan.bottom, try number(ground, "bottom_row"))
        XCTAssertEqual(SceneryFraming.skyBottomRow, try number(sky, "bottom_row"))
    }

    func testTheHeroHasTheMockupProportionsAndFadesAlongItsCurve() throws {
        let fixture = try fixture()
        XCTAssertEqual(SceneryFraming.heroHeightRatio, try number(fixture, "hero_height_ratio"), accuracy: 0.000001)
        XCTAssertEqual(SceneryFraming.clearBottomFraction, try number(fixture, "clear_bottom_fraction"))
        let stops = try XCTUnwrap(fixture["mask_stops"] as? [[NSNumber]], "mask_stops")
        XCTAssertEqual(SceneryFraming.maskStops.map(\.at), stops.map { CGFloat($0[0].doubleValue) })
        XCTAssertEqual(SceneryFraming.maskStops.map(\.alpha), stops.map { CGFloat($0[1].doubleValue) })
    }

    /// The fade given to the formula ends the clear zone where the mask reaches half opacity.
    func testTheClearZoneEndsWhereTheFadeReachesHalfOpacity() throws {
        for testCase in try ClientRulesFixtures.cases(try fixture(), "cases") {
            let name = try ClientRulesFixtures.string(testCase, "name")
            let hero = try numbers(testCase, "hero_px")
            XCTAssertEqual(
                SceneryFraming.fade(forHeroHeight: hero[1]), try number(testCase, "fade_px"), accuracy: 0.01, name)
        }
    }

    /// 300 high on the mockup's 360 width, plus the status bar the hero draws under on a phone.
    func testTheHeroIsTheColumnWidthAtTheMockupRatioPlusTheStatusBar() {
        XCTAssertEqual(SceneryFraming.heroHeight(columnWidth: 360, safeTop: 0), 300, accuracy: 0.001)
        XCTAssertEqual(SceneryFraming.heroHeight(columnWidth: 412, safeTop: 24), 367.33, accuracy: 0.01)
    }

    /// The controller frames the canvas with the fade of its own height, not a fixed band: the
    /// desktop window's hero lands exactly where the fixture puts it.
    func testTheHeroControllerFramesWithTheFadeOfItsHeight() throws {
        let controller = SceneryViewController()
        controller.loadViewIfNeeded()
        controller.view.frame = CGRect(x: 0, y: 0, width: 360, height: 300)
        controller.update(phase: .exposed, exitCountry: nil, animated: false)
        controller.view.layoutIfNeeded()
        let frame = try XCTUnwrap(controller.canvasFrame)
        XCTAssertEqual(frame.top, -31.11, accuracy: Self.tolerance)
        XCTAssertEqual(frame.y(ofRow: SceneryFraming.groundSpan.bottom), 231.0, accuracy: Self.tolerance)
    }

    /// The hero's alpha follows the mockup curve from its top to its bottom.
    func testTheHeroMaskIsTheMockupCurve() throws {
        let controller = SceneryViewController()
        controller.loadViewIfNeeded()
        controller.view.frame = CGRect(x: 0, y: 0, width: 393, height: 389)
        controller.view.layoutIfNeeded()
        let mask = try XCTUnwrap(controller.view.mask as? GradientView)
        XCTAssertEqual(mask.frame, controller.view.bounds)
        let locations = try XCTUnwrap(mask.gradientLayer.locations).map { CGFloat($0.doubleValue) }
        XCTAssertEqual(locations, SceneryFraming.maskStops.map(\.at))
        let colors = try XCTUnwrap(mask.gradientLayer.colors as? [CGColor])
        XCTAssertEqual(colors.map(\.alpha), SceneryFraming.maskStops.map(\.alpha))
    }

    func testEveryCaseIsFramedAsTheFixtureExpects() throws {
        let cases = try ClientRulesFixtures.cases(try fixture(), "cases")
        XCTAssertFalse(cases.isEmpty)
        for testCase in cases where !ClientRulesFixtures.skippedOnIOS(testCase) {
            let name = try ClientRulesFixtures.string(testCase, "name")
            let expect = try ClientRulesFixtures.object(testCase, "expect")
            let (frame, span) = try frame(for: testCase)
            let tolerance = Self.tolerance
            XCTAssertEqual(frame.scale, try number(expect, "scale"), accuracy: 0.0001, "\(name) scale")
            XCTAssertEqual(frame.left, try number(expect, "canvas_left_px"), accuracy: tolerance, "\(name) left")
            XCTAssertEqual(frame.top, try number(expect, "canvas_top_px"), accuracy: tolerance, "\(name) top")
            XCTAssertEqual(frame.width, try number(expect, "canvas_width_px"), accuracy: tolerance, "\(name) width")
            XCTAssertEqual(
                frame.height, try number(expect, "canvas_height_px"), accuracy: tolerance, "\(name) height")
            XCTAssertEqual(
                frame.headroom, try number(expect, "headroom_px"), accuracy: tolerance, "\(name) headroom")
            XCTAssertEqual(
                frame.y(ofRow: span.top), try number(expect, "span_top_px"), accuracy: tolerance, "\(name) span top")
            XCTAssertEqual(
                frame.y(ofRow: span.bottom), try number(expect, "span_bottom_px"), accuracy: tolerance,
                "\(name) span bottom")
        }
    }

    func testEverySpanStaysBetweenTheHeaderAndTheFade() throws {
        for testCase in try ClientRulesFixtures.cases(try fixture(), "cases") {
            let name = try ClientRulesFixtures.string(testCase, "name")
            let hero = try numbers(testCase, "hero_px")
            let (frame, span) = try frame(for: testCase)
            XCTAssertGreaterThanOrEqual(
                frame.y(ofRow: span.top), try number(testCase, "clear_top_px") - 0.01, name)
            XCTAssertLessThanOrEqual(
                frame.y(ofRow: span.bottom), hero[1] - (try number(testCase, "fade_px")) + 0.01, name)
        }
    }

    func testTheThumbnailIsCrownedByTheSkyline() throws {
        let thumbnail = try ClientRulesFixtures.object(try fixture(), "thumbnail")
        XCTAssertEqual(SceneryFraming.thumbMarginRows, try number(thumbnail, "thumb_margin_rows"))
        let cases = try ClientRulesFixtures.cases(thumbnail, "cases")
        XCTAssertFalse(cases.isEmpty)
        for thumbCase in cases {
            let skyline = try ClientRulesFixtures.int(thumbCase, "skyline_top_row")
            XCTAssertEqual(
                SceneryFraming.thumbnailTopRow(skylineTopRow: skyline),
                CGFloat(try ClientRulesFixtures.int(thumbCase, "top_row")),
                "skyline \(skyline)"
            )
        }
    }

    /// The framing column of scenery.json, through the generated tables: the ground whenever Bula
    /// is out, the country's own skyline once he is in, and the plain's skyline for a country
    /// without art.
    func testEachPhaseFramesTheSpanItsSceneryRowNames() {
        XCTAssertEqual(SceneryViewController.framingSpan(.exposed, country: "Netherlands"), SceneryFraming.groundSpan)
        XCTAssertEqual(
            SceneryViewController.framingSpan(.connecting, country: "Netherlands"), SceneryFraming.groundSpan)
        XCTAssertEqual(
            SceneryViewController.framingSpan(.protected, country: "Netherlands"),
            SceneryFraming.Span(top: 100, bottom: 610))
        XCTAssertEqual(
            SceneryViewController.framingSpan(.protected, country: "nl"),
            SceneryFraming.Span(top: 100, bottom: 610))
        XCTAssertEqual(
            SceneryViewController.framingSpan(.protected, country: "Sweden"),
            SceneryFraming.Span(top: 313, bottom: 610))
        // The kill switch shows the watched plain whatever the exit.
        XCTAssertEqual(SceneryViewController.framingSpan(.blocked, country: "Finland"), SceneryFraming.groundSpan)
    }

    func testOnlyTheCountriesWithArtHaveALandscapeOfTheirOwn() {
        XCTAssertEqual(SceneryViewController.landscapeImageName(forCountry: "Finland"), "SceneryFinland")
        XCTAssertEqual(SceneryViewController.landscapeImageName(forCountry: " SG "), "ScenerySingapore")
        XCTAssertEqual(SceneryViewController.landscapeImageName(forCountry: "Sweden"), "SceneryPlaine")
        XCTAssertEqual(SceneryViewController.landscapeImageName(forCountry: nil), "SceneryPlaine")
        XCTAssertTrue(SceneryViewController.hasCountryArt("france"))
        XCTAssertFalse(SceneryViewController.hasCountryArt("se"))
    }
}
