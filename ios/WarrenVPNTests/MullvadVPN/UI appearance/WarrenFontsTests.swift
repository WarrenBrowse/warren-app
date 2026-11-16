//
//  WarrenFontsTests.swift
//  WarrenVPNTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import UIKit
import XCTest

@testable import WarrenVPN

/// The bundled faces load from the app bundle under the names the code asks for, and the sizes
/// built from them keep following Dynamic Type.
final class WarrenFontsTests: XCTestCase {
    func testEveryBundledFaceIsRegistered() {
        for face in WarrenFontFace.allCases {
            XCTAssertNotNil(UIFont(name: face.rawValue, size: 12), "\(face.rawValue) is not in the bundle")
        }
    }

    func testTheNamedStylesAreSetInInstrumentSans() {
        XCTAssertEqual(UIFont.warrenSmall.familyName, "Instrument Sans")
        XCTAssertEqual(UIFont.warrenLarge.fontName, WarrenFontFace.bold.rawValue)
        XCTAssertEqual(UIFont.jetBrainsMono(size: 12).familyName, "JetBrains Mono")
    }

    func testEachWeightTakesTheNearestBundledFace() {
        XCTAssertEqual(WarrenFontFace.sans(.light), .regular)
        XCTAssertEqual(WarrenFontFace.sans(.regular), .regular)
        XCTAssertEqual(WarrenFontFace.sans(.medium), .medium)
        XCTAssertEqual(WarrenFontFace.sans(.semibold), .semibold)
        XCTAssertEqual(WarrenFontFace.sans(.bold), .bold)
        XCTAssertEqual(WarrenFontFace.sans(.black), .bold)
        XCTAssertEqual(WarrenFontFace.mono(.regular), .monoRegular)
        XCTAssertEqual(WarrenFontFace.mono(.semibold), .monoMedium)
    }

    /// A text style's face starts at that style's own size, so at the default content size the
    /// screen keeps the measurements it was designed at.
    func testATextStyleFaceStartsAtThatStylesSize() {
        let base = UITraitCollection(preferredContentSizeCategory: .large)
        XCTAssertEqual(UIFont.defaultSize(of: .footnote), 13)
        XCTAssertEqual(UIFont.defaultSize(of: .largeTitle), 34)
        let footnote = UIFont.instrumentSans(.footnote)
        XCTAssertEqual(footnote.fontName, WarrenFontFace.regular.rawValue)
        XCTAssertEqual(
            UIFontMetrics(forTextStyle: .footnote).scaledValue(for: 13, compatibleWith: base),
            13, accuracy: 0.01)
    }
}
