//
//  SceneryLayoutTests.swift
//  WarrenVPNTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import XCTest

@testable import WarrenVPN

final class SceneryLayoutTests: XCTestCase {
    // The 1140x1706 scenery art on a tall phone: the full painted width
    // must be visible (edge elements like the country flag included),
    // anchored to the TOP so the real painted sky stays sharp behind the
    // header, with the bottom extension owning the strip below.
    func testTallPhoneShowsFullArtWidthWithBottomGapBelow() {
        let bounds = CGRect(x: 0, y: 0, width: 393, height: 852)

        let layout = SceneryViewController.landscapeLayout(
            imageSize: CGSize(width: 1140, height: 1706), in: bounds)

        let expectedHeight = 393.0 * 1706.0 / 1140.0
        XCTAssertEqual(layout.landscape.width, 393)
        XCTAssertEqual(layout.landscape.minX, 0)
        XCTAssertEqual(layout.landscape.minY, 0)
        XCTAssertEqual(layout.landscape.height, expectedHeight, accuracy: 0.001)

        XCTAssertEqual(layout.bottomExtension.maxY, 852)
        XCTAssertEqual(layout.bottomExtension.width, 393)
        // 1pt overlap over the art bottom edge prevents a hairline gap.
        XCTAssertEqual(
            layout.bottomExtension.minY, expectedHeight - 1, accuracy: 0.001)
    }

    // A screen wider than the art aspect is already covered by the
    // width-fit art (its bottom crops offscreen): no extension.
    func testWideScreenNeedsNoBottomExtension() {
        let bounds = CGRect(x: 0, y: 0, width: 1024, height: 768)

        let layout = SceneryViewController.landscapeLayout(
            imageSize: CGSize(width: 1140, height: 1706), in: bounds)

        XCTAssertEqual(layout.bottomExtension, .zero)
        XCTAssertEqual(layout.landscape.width, 1024)
        XCTAssertEqual(layout.landscape.minY, 0)
        XCTAssertGreaterThan(layout.landscape.maxY, 768)
    }

    func testMissingImageFallsBackToFullBounds() {
        let bounds = CGRect(x: 0, y: 0, width: 393, height: 852)

        let layout = SceneryViewController.landscapeLayout(imageSize: nil, in: bounds)

        XCTAssertEqual(layout.landscape, bounds)
        XCTAssertEqual(layout.bottomExtension, .zero)
    }
}
