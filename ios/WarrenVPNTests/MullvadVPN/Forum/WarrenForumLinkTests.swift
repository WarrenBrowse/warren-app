//
//  WarrenForumLinkTests.swift
//  WarrenVPNTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import WarrenRustRuntime
import XCTest

@testable import WarrenVPN

/// The deep-link rules replayed from `fixtures/client-rules/forum_link.json`
/// with the full rejection-class vocabulary, the same file the Rust crate, the
/// JVM suite and the desktop suite read on their side.
final class WarrenForumLinkTests: XCTestCase {
    private let allowedHost = "connect.warrenbrowse.com"

    func testTheSharedLinkFixtureReplaysCaseForCase() throws {
        let fixture = try ClientRulesFixtures.load("forum_link.json")
        let cases = try ClientRulesFixtures.cases(fixture, "login_cases").filter {
            !ClientRulesFixtures.skippedOnIOS($0)
        }
        XCTAssertGreaterThanOrEqual(cases.count, 20, "only \(cases.count) login cases reached this reader")
        let hosts = try XCTUnwrap(fixture["allowed_hosts"] as? [String])
        XCTAssertEqual(hosts, [allowedHost])
        for testCase in cases {
            let name = try ClientRulesFixtures.string(testCase, "name")
            let verdict = WarrenForumLinks.classify(
                testCase["url"] as? String,
                expectedScheme: try ClientRulesFixtures.string(testCase, "expected_scheme"),
                allowedHost: allowedHost)
            let expect = try ClientRulesFixtures.object(testCase, "expect")
            if let accepted = expect["accepted"] as? [String: Any] {
                let link = ForumLoginLink(
                    sid: try ClientRulesFixtures.string(accepted, "sid"),
                    host: try ClientRulesFixtures.string(accepted, "host"),
                    crossDevice: try XCTUnwrap(accepted["cross_device"] as? Bool))
                XCTAssertEqual(verdict, .accepted(link), name)
            } else {
                XCTAssertEqual(verdict, .rejected(try ClientRulesFixtures.string(expect, "rejected")), name)
            }
        }
    }

    func testTheSchemesOfTheFixtureAreTheProductTables() throws {
        // The scheme this build answers is the Rust table's; the fixture
        // spells every environment's so a link case can be read on its own.
        let link = try ClientRulesFixtures.load("forum_link.json")
        let env = try ClientRulesFixtures.load("product_env.json")
        let schemes = try ClientRulesFixtures.object(link, "schemes")
        let environments = try ClientRulesFixtures.object(env, "environments")
        for (name, scheme) in schemes {
            let row = try ClientRulesFixtures.object(environments, name)
            XCTAssertEqual(scheme as? String, try ClientRulesFixtures.string(row, "deep_link_scheme"), name)
        }
        let current = WarrenProductAnchors.current
        XCTAssertEqual(schemes[current.name] as? String, current.deepLinkScheme)
    }

    func testTheSignInCodeCasesReplayThroughTheNormaliser() throws {
        let fixture = try ClientRulesFixtures.load("forum_link.json")
        for testCase in try ClientRulesFixtures.cases(fixture, "sign_in_code_cases") {
            let name = try ClientRulesFixtures.string(testCase, "name")
            let typed = try ClientRulesFixtures.string(testCase, "typed")
            XCTAssertEqual(WarrenForumLinks.normalizeSignInCode(typed), testCase["expect"] as? String, name)
        }
    }

    func testATypedCodeCarriesTheCrossDeviceWarningNoSignalCanPlace() throws {
        // A typed code carries no `xd`, and by construction there is no link:
        // the code can as easily have been read off another screen or pasted
        // into a chat by whoever started the sign-in. Only the cross-device
        // prompt says that approving signs in whoever sent it.
        let sid = "0123456789abcdef0123456789abcdef"
        XCTAssertEqual(
            WarrenForumLinks.linkFromCode(sid, host: allowedHost),
            ForumLoginLink(sid: sid, host: allowedHost, crossDevice: true))
        let fixture = try ClientRulesFixtures.load("forum_link.json")
        XCTAssertEqual(fixture["sign_in_code_cross_device"] as? Bool, true)
    }

    func testTheSceneHandsTheFlowAURLAsItsAbsoluteString() throws {
        // The scene receives a `URL`; the classifier reads the same string a
        // cold-start context carries, so both entry points see one rule.
        let url = try XCTUnwrap(
            URL(string: "warren-beta://forum-login?sid=0123456789abcdef0123456789abcdef&host=\(allowedHost)&xd=1"))
        XCTAssertEqual(
            WarrenForumLinks.classify(url.absoluteString, expectedScheme: "warren-beta", allowedHost: allowedHost),
            .accepted(ForumLoginLink(sid: "0123456789abcdef0123456789abcdef", host: allowedHost, crossDevice: true)))
        XCTAssertEqual(
            WarrenForumLinks.classify(url.absoluteString, expectedScheme: "warren", allowedHost: allowedHost),
            .rejected("wrong-scheme:warren-beta"))
    }
}
