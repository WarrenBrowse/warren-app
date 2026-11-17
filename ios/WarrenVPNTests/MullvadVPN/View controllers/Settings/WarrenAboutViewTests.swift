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
@testable import WarrenRustRuntime
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

    /// The title names the product this build IS, from the compiled table. A
    /// literal read "Warren VPN" on every environment, so the one screen a user
    /// opens to check what they are running answered for prod on a beta
    /// install.
    func test_title_isTheDisplayNameOfTheCompiledEnvironment() {
        let view = WarrenAboutView(appVersion: "1.0.0", buildNumber: "1")
        XCTAssertEqual(view.productName, WarrenProductAnchors.current.displayName)
    }

    func test_title_followsTheEnvironmentItIsGiven() throws {
        let fixture = try ClientRulesFixtures.load("product_env.json")
        let environments = try ClientRulesFixtures.object(fixture, "environments")
        for name in environments.keys {
            let row = try ClientRulesFixtures.object(environments, name)
            let data = try JSONSerialization.data(withJSONObject: row)
            let anchors = try XCTUnwrap(
                WarrenProductAnchors.decode(String(decoding: data, as: UTF8.self)))
            let view = WarrenAboutView(appVersion: "1.0.0", buildNumber: "1", anchors: anchors)
            XCTAssertEqual(
                view.productName,
                try ClientRulesFixtures.string(row, "display_name"),
                name
            )
        }
    }
}

/// The app is distributed under the GPL, version 3 or later; only its tunnel
/// engine is under the AGPL. The About screen used to call the app's own
/// source AGPL-3.0.
final class WarrenLicensesViewTests: XCTestCase {
    func test_theAppIsPresentedUnderTheGPLAndOnlyTheEngineUnderTheAGPL() {
        let paragraphs = WarrenLicensesView.paragraphs

        XCTAssertTrue(paragraphs[0].contains("GNU General Public License, version 3 or any later version"))
        XCTAssertTrue(paragraphs[1].contains("Mullvad VPN AB"))
        XCTAssertTrue(paragraphs[2].contains("warrenguard"))
        XCTAssertTrue(paragraphs[2].contains("Affero"))
        XCTAssertFalse(paragraphs[0].contains("Affero"))
    }

    func test_theSourceLinksNameThisBuildsCodeAndItsEngine() {
        let urls = WarrenLicensesView.sourceLinks.map(\.url)

        XCTAssertEqual(urls, [WarrenSourceLicense.appSourceURL, WarrenSourceLicense.engineSourceURL])
        XCTAssertTrue(urls[0].hasPrefix(WarrenSourceLicense.appRepositoryURL))
        XCTAssertTrue(urls[1].hasPrefix(WarrenSourceLicense.engineRepositoryURL + "/tree/"))
    }
}
