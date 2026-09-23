//
//  WarrenForumLinkTests.swift
//  WarrenVPNTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import WarrenRustRuntime
import XCTest

@testable import WarrenVPN

/// The deep-link rules (the login's and the attach-logs one's) replayed from
/// `fixtures/client-rules/forum_link.json` with the full rejection-class
/// vocabulary, the same file the Rust crate, the JVM suite and the desktop
/// suite read on their side.
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

    func testTheSharedAttachFixtureReplaysCaseForCase() throws {
        let fixture = try ClientRulesFixtures.load("forum_link.json")
        let cases = try ClientRulesFixtures.cases(fixture, "attach_cases").filter {
            !ClientRulesFixtures.skippedOnIOS($0)
        }
        XCTAssertGreaterThanOrEqual(cases.count, 10, "only \(cases.count) attach cases reached this reader")
        for testCase in cases {
            let name = try ClientRulesFixtures.string(testCase, "name")
            let verdict = WarrenForumLinks.classifyAttach(
                testCase["url"] as? String,
                expectedScheme: try ClientRulesFixtures.string(testCase, "expected_scheme"),
                allowedHost: allowedHost)
            let expect = try ClientRulesFixtures.object(testCase, "expect")
            if let accepted = expect["accepted"] as? [String: Any] {
                let link = ForumAttachLink(
                    sid: try ClientRulesFixtures.string(accepted, "sid"),
                    host: try ClientRulesFixtures.string(accepted, "host"),
                    topicId: UInt64(truncating: try XCTUnwrap(accepted["topic_id"] as? NSNumber)))
                XCTAssertEqual(verdict, .accepted(link), name)
            } else {
                XCTAssertEqual(verdict, .rejected(try ClientRulesFixtures.string(expect, "rejected")), name)
            }
        }
    }

    func testTheActionRoutesALinkBeforeEitherParserRuns() throws {
        // One URL scheme serves both flows, so the scene picks the parser by
        // the action alone; a link neither flow owns still reaches the login
        // parser, which rejects it by class as it always did.
        let query = "sid=0123456789abcdef0123456789abcdef&host=\(allowedHost)"
        XCTAssertEqual(WarrenForumLinks.action(of: "warren://attach-logs?topic=1&\(query)"), "attach-logs")
        XCTAssertEqual(WarrenForumLinks.action(of: "warren://forum-login?\(query)"), "forum-login")
        XCTAssertEqual(WarrenForumLinks.action(of: "warren-beta:///attach-logs?\(query)"), "attach-logs")
        XCTAssertNil(WarrenForumLinks.action(of: "::not a uri::"))
        XCTAssertEqual(WarrenForumLinks.attachAction, "attach-logs")
        XCTAssertEqual(WarrenForumLinks.loginAction, "forum-login")
    }

    func testATypedCodeStandsForAnAttachWithTheTopicItsMetaNamed() throws {
        // The attach page prints its session id in the shape of the sign-in
        // code, and the topic is the one thing the code cannot carry: the
        // broker's meta names it (0 for a report still being composed), so
        // the link a typed code stands for carries it like a deep link.
        let sid = "0123456789abcdef0123456789abcdef"
        XCTAssertEqual(
            WarrenForumLinks.attachLinkFromCode(sid, host: allowedHost, topicId: 4242),
            ForumAttachLink(sid: sid, host: allowedHost, topicId: 4242))
        XCTAssertTrue(WarrenForumLinks.attachLinkFromCode(sid, host: allowedHost, topicId: 0).isPreTopic)
        XCTAssertFalse(ForumAttachLink(sid: sid, host: allowedHost, topicId: 42).isPreTopic)
        XCTAssertEqual(ForumAttachLink.preTopic, 0)
        XCTAssertEqual(WarrenForumLinks.parseTopicId("042"), 42)
        XCTAssertNil(WarrenForumLinks.parseTopicId("4 2"))
        XCTAssertNil(WarrenForumLinks.parseTopicId("9007199254740993"))
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
            ForumLoginLink(sid: sid, host: allowedHost, crossDevice: true, typedCode: true))
        let fixture = try ClientRulesFixtures.load("forum_link.json")
        XCTAssertEqual(fixture["sign_in_code_cross_device"] as? Bool, true)
    }

    func testEachWayInIsItsOwnApproach() {
        // The completion screen differs by approach: a typed code shows the
        // code and keeps the handoff behind a button, a same-device link opens
        // it, a QR never does.
        let sid = "0123456789abcdef0123456789abcdef"
        let good = "warren://forum-login?sid=\(sid)&host=\(allowedHost)"
        guard
            case .accepted(let button) = WarrenForumLinks.classify(
                good, expectedScheme: "warren", allowedHost: allowedHost),
            case .accepted(let qr) = WarrenForumLinks.classify(
                good + "&xd=1", expectedScheme: "warren", allowedHost: allowedHost)
        else { return XCTFail("the links are accepted") }
        XCTAssertEqual(ForumLoginApproach.of(button), .sameDeviceLink)
        XCTAssertEqual(ForumLoginApproach.of(qr), .crossDeviceLink)
        XCTAssertEqual(ForumLoginApproach.of(WarrenForumLinks.linkFromCode(sid, host: allowedHost)), .typedCode)
    }

    func testEveryApproachLeadsToTheScreenAndTheHandoffTheFixtureNames() throws {
        let outcomes = try ClientRulesFixtures.load("forum_outcomes.json")
        let login = try ClientRulesFixtures.object(outcomes, "login")
        let completion = try ClientRulesFixtures.object(login, "completion")
        let answers = try Dictionary(
            uniqueKeysWithValues: ClientRulesFixtures.cases(login, "cases").map {
                (try ClientRulesFixtures.string($0, "name"), $0)
            })
        let approvedSid = try ClientRulesFixtures.string(login, "sid")
        XCTAssertEqual(
            Set(try XCTUnwrap(completion["approaches"] as? [String])),
            Set(ForumLoginApproach.allCases.map(\.rawValue)))
        let cases = try ClientRulesFixtures.cases(completion, "cases").filter { !ClientRulesFixtures.skippedOnIOS($0) }
        XCTAssertGreaterThanOrEqual(cases.count, 9, "only \(cases.count) completion cases reached this reader")
        for testCase in cases {
            let name = try ClientRulesFixtures.string(testCase, "name")
            let answer = try XCTUnwrap(answers[try ClientRulesFixtures.string(testCase, "answer")], name)
            let approach = try XCTUnwrap(
                ForumLoginApproach(rawValue: try ClientRulesFixtures.string(testCase, "approach")), name)
            guard
                case .approved(_, let decoded) = WarrenAccountClient.forumLoginOutcome(
                    fromEnvelope: try ClientRulesFixtures.string(answer, "envelope"), sid: approvedSid,
                    connectHost: allowedHost)
            else { return XCTFail("\(name): the answer is an approval") }
            let plan = WarrenForumLinks.completionPlan(approach: approach, completion: decoded)
            let expect = try ClientRulesFixtures.object(testCase, "expect")
            XCTAssertEqual(plan.screen.rawValue, try ClientRulesFixtures.string(expect, "screen"), name)
            XCTAssertEqual(plan.handoff.rawValue, try ClientRulesFixtures.string(expect, "handoff"), name)
        }
        XCTAssertEqual(
            WarrenForumLinks.codeLifetime,
            TimeInterval(try XCTUnwrap(completion["code_lifetime_secs"] as? Int)))
    }

    func testTheCompletionSessionHandsEachHandoffOverOnceAndNotAfterTheSessionDied() {
        let sid = "0123456789abcdef0123456789abcdef"
        let handoff = "https://\(allowedHost)/handoff#sid=\(sid)&code=042917"
        let completion = WarrenForumLoginCompletion(code: "042917", handoffURL: handoff)
        let received = Date(timeIntervalSince1970: 1_000)
        let link = ForumLoginLink(sid: sid, host: allowedHost, crossDevice: false)

        let sameDevice = WarrenForumCompletionSession(link: link, completion: completion, receivedAt: received)
        XCTAssertEqual(sameDevice.screen, .finishingInBrowser)
        XCTAssertEqual(sameDevice.takeHandoffToOpen(at: received), handoff)
        XCTAssertNil(sameDevice.takeHandoffToOpen(at: received))
        XCTAssertEqual(sameDevice.offersFinishInBrowser, false)

        let typed = WarrenForumCompletionSession(
            link: WarrenForumLinks.linkFromCode(sid, host: allowedHost), completion: completion, receivedAt: received)
        XCTAssertEqual(typed.screen, .showCode)
        XCTAssertNil(typed.takeHandoffToOpen(at: received))
        XCTAssertEqual(typed.offersFinishInBrowser, true)
        let late = received.addingTimeInterval(WarrenForumLinks.codeLifetime)
        XCTAssertEqual(typed.isExpired(at: late), true)
        XCTAssertNil(typed.takeFinishURL(at: late))

        let qr = WarrenForumCompletionSession(
            link: ForumLoginLink(sid: sid, host: allowedHost, crossDevice: true), completion: completion,
            receivedAt: received)
        XCTAssertNil(qr.takeHandoffToOpen(at: received))
        XCTAssertEqual(qr.offersFinishInBrowser, false)
        XCTAssertFalse(String(describing: qr).contains("042917"))
    }

    func testASameDeviceLinkWithoutAHandoffShowsTheCodeUnderTheRelayWarning() {
        let sid = "0123456789abcdef0123456789abcdef"
        let link = ForumLoginLink(sid: sid, host: allowedHost, crossDevice: false)

        let session = WarrenForumCompletionSession(
            link: link, completion: WarrenForumLoginCompletion(code: "042917", handoffURL: nil),
            receivedAt: Date(timeIntervalSince1970: 1_000))

        XCTAssertEqual(session.screen, .showCodeRelayed)
        XCTAssertTrue(session.revealsCodeAtOnce)
        XCTAssertNil(session.takeHandoffToOpen(at: Date(timeIntervalSince1970: 1_000)))
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
