//
//  WarrenAboutViewTests.swift
//  WarrenVPNTests
//
//  Created by Warren on 2026-05-22.
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  Smoke tests for `WarrenAboutView`. The view's main responsibility
//  is to render 4 external links + a version banner ; we test the
//  init wiring + that the version+build strings round-trip exactly
//  as passed (no silent reformatting that would surprise users
//  comparing against TestFlight / App Store metadata).
//

import SwiftUI
import XCTest
@testable import WarrenVPN


final class WarrenAboutViewTests: XCTestCase {
    /// Init stores the version + build exactly as passed (no
    /// reformatting). Important : App Store + Settings.app surface
    /// these strings ; mismatch would confuse support.
    func test_init_preservesVersionAndBuildVerbatim() {
        let view = WarrenAboutView(appVersion: "0.5.2", buildNumber: "1042")
        XCTAssertEqual(view.appVersion, "0.5.2")
        XCTAssertEqual(view.buildNumber, "1042")
    }

    /// Empty version / build strings are rendered as-is (no
    /// substitution to "unknown" etc.) - the factory upstream
    /// substitutes "?" when the Info.plist key is missing, so this
    /// view stays dumb.
    func test_init_acceptsEmptyStrings() {
        let view = WarrenAboutView(appVersion: "", buildNumber: "")
        XCTAssertEqual(view.appVersion, "")
        XCTAssertEqual(view.buildNumber, "")
    }

    /// Init with semantic-version-like strings (the common case).
    func test_init_acceptsSemverLikeStrings() {
        let view = WarrenAboutView(appVersion: "1.0.0-beta.3", buildNumber: "42")
        XCTAssertEqual(view.appVersion, "1.0.0-beta.3")
        XCTAssertEqual(view.buildNumber, "42")
    }
}
