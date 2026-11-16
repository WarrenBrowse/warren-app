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
            span: span,
            compact: testCase["compact"] as? Bool ?? false,
            fullHeight: (testCase["full_hero_height_px"] as? NSNumber).map { CGFloat($0.doubleValue) }
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
        XCTAssertEqual(SceneryFraming.heroMaxHeightRatio, try number(fixture, "hero_max_height_ratio"))
        XCTAssertEqual(
            SceneryFraming.compactHeightRatio, try number(fixture, "compact_height_ratio"), accuracy: 0.000001)
        XCTAssertEqual(SceneryFraming.compactTransitionMs, try number(fixture, "compact_transition_ms"))
        XCTAssertEqual(SceneryFraming.marginBlur, try number(fixture, "margin_blur_px"))
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

    /// 300 high on the mockup's 380 width, plus the status bar the hero draws under on a phone.
    func testTheHeroIsTheColumnWidthAtTheMockupRatioPlusTheStatusBar() {
        XCTAssertEqual(SceneryFraming.heroHeight(columnWidth: 380, safeTop: 0), 300, accuracy: 0.001)
        XCTAssertEqual(SceneryFraming.heroHeight(columnWidth: 412, safeTop: 24), 349.26, accuracy: 0.01)
    }

    /// The mask's half-opacity point is where the clear zone ends, so the formula's fade and the
    /// drawn fade agree.
    func testTheClearBottomIsWhereTheMaskCrossesHalfOpacity() {
        let stops = SceneryFraming.maskStops
        let at = SceneryFraming.clearBottomFraction
        let index = stops.firstIndex { $0.at >= at }!
        let (low, high) = (stops[index - 1], stops[index])
        let alpha = low.alpha + (high.alpha - low.alpha) * (at - low.at) / (high.at - low.at)
        XCTAssertEqual(alpha, 0.5, accuracy: 0.000001)
    }

    /// A narrowed canvas is backed by the same landscape covering the whole hero, its span's middle
    /// at the height the canvas draws it so the horizon runs on across the join.
    func testANarrowedCanvasIsBackedByTheLandscapeAtCoverScale() throws {
        let span = SceneryFraming.groundSpan
        let frame = SceneryFraming.frame(width: 1024, height: 400, clearTop: 68, fade: 48, span: span)
        let backdrop = try XCTUnwrap(SceneryFraming.marginBackdrop(width: 1024, height: 400, frame: frame, span: span))
        XCTAssertEqual(backdrop.width, 1024, accuracy: 0.01)
        XCTAssertLessThanOrEqual(backdrop.minY, 0)
        XCTAssertGreaterThanOrEqual(backdrop.maxY, 400)
        let middle = (span.top + span.bottom) / 2
        XCTAssertEqual(
            backdrop.minY + middle * backdrop.width / SceneryFraming.canvasWidth, frame.y(ofRow: middle),
            accuracy: 0.01)
    }

    func testACanvasAtFullWidthHasNoBackdrop() {
        let span = SceneryFraming.groundSpan
        let frame = SceneryFraming.frame(width: 380, height: 300, clearTop: 68, fade: 36, span: span)
        XCTAssertNil(SceneryFraming.marginBackdrop(width: 380, height: 300, frame: frame, span: span))
    }

    /// The controller frames the canvas with the fade of its own height, not a fixed band: the
    /// desktop window's hero lands exactly where the fixture puts it.
    func testTheHeroControllerFramesWithTheFadeOfItsHeight() throws {
        let controller = SceneryViewController()
        controller.loadViewIfNeeded()
        controller.view.frame = CGRect(x: 0, y: 0, width: 380, height: 300)
        controller.update(phase: .exposed, exitCountry: nil, animated: false)
        controller.view.layoutIfNeeded()
        let frame = try XCTUnwrap(controller.canvasFrame)
        XCTAssertEqual(frame.top, -36.0, accuracy: Self.tolerance)
        XCTAssertEqual(frame.y(ofRow: SceneryFraming.groundSpan.bottom), 264.0, accuracy: Self.tolerance)
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

    func testEveryFullHeightSpanStaysBetweenTheHeaderAndTheFade() throws {
        let cases = try ClientRulesFixtures.cases(try fixture(), "cases")
        for testCase in cases where testCase["compact"] as? Bool != true {
            let name = try ClientRulesFixtures.string(testCase, "name")
            let hero = try numbers(testCase, "hero_px")
            let (frame, span) = try frame(for: testCase)
            XCTAssertGreaterThanOrEqual(
                frame.y(ofRow: span.top), try number(testCase, "clear_top_px") - 0.01, name)
            XCTAssertLessThanOrEqual(
                frame.y(ofRow: span.bottom), hero[1] - (try number(testCase, "fade_px")) + 0.01, name)
        }
    }

    /// Opening the details slides the art up at the scale the same hero has at full height: it
    /// never zooms, on a wide screen either.
    func testACompactHeroKeepsItsFullHeightScaleAndPinsTheSpanTopUnderTheHeader() throws {
        let cases = try ClientRulesFixtures.cases(try fixture(), "cases").filter { $0["compact"] as? Bool == true }
        XCTAssertFalse(cases.isEmpty)
        for testCase in cases {
            let name = try ClientRulesFixtures.string(testCase, "name")
            let hero = try numbers(testCase, "hero_px")
            let fullHeight = try number(testCase, "full_hero_height_px")
            let (frame, span) = try frame(for: testCase)
            let full = SceneryFraming.frame(
                width: hero[0],
                height: fullHeight,
                clearTop: try number(testCase, "clear_top_px"),
                fade: SceneryFraming.fade(forHeroHeight: fullHeight),
                span: span
            )
            XCTAssertEqual(frame.scale, full.scale, accuracy: 0.000001, name)
            XCTAssertEqual(frame.left, full.left, accuracy: 0.000001, name)
            XCTAssertEqual(frame.y(ofRow: span.top), try number(testCase, "clear_top_px"), accuracy: 0.000001, name)
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
            SceneryFraming.Span(top: 100, bottom: 680))
        XCTAssertEqual(
            SceneryViewController.framingSpan(.protected, country: "nl"),
            SceneryFraming.Span(top: 100, bottom: 680))
        XCTAssertEqual(
            SceneryViewController.framingSpan(.protected, country: "Sweden"),
            SceneryFraming.Span(top: 313, bottom: 680))
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
