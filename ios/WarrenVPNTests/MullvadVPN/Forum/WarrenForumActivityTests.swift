//
//  WarrenForumActivityTests.swift
//  WarrenVPNTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import XCTest

@testable import WarrenVPN

/// The forum badge rules replayed from
/// `fixtures/client-rules/forum_activity.json`, the file the desktop and
/// Android readers replay too. A rule change means changing that file and
/// every reader in the same commit; a reader is never loosened to pass.
@MainActor
final class WarrenForumActivityTests: XCTestCase {
    private var fixture: [String: Any]!

    override func setUp() async throws {
        try await super.setUp()
        fixture = try ClientRulesFixtures.load("forum_activity.json")
    }

    func testTheSaturationCeilingIsTheOneEverySurfaceCountsTo() throws {
        XCTAssertEqual(try ClientRulesFixtures.int(fixture, "unread_saturated"), warrenUnreadSaturated)
    }

    func testEveryDigestIsIndexedTheWayTheFixtureStates() throws {
        let cases = try ClientRulesFixtures.cases(fixture, "unread_for_slot_cases")
        XCTAssertGreaterThanOrEqual(cases.count, 6, "only \(cases.count) indexing cases reached this reader")
        for testCase in cases {
            let name = try ClientRulesFixtures.string(testCase, "name")
            let unread = WarrenForumActivity.unread(
                in: testCase["digest"] as? String,
                slot: (testCase["slot"] as? NSNumber)?.intValue)
            let expected = try ClientRulesFixtures.int(testCase, "expect")
            XCTAssertEqual(unread, expected, name)
        }
    }

    func testTheLabelSaturatesRatherThanGrowingTheBadge() throws {
        for testCase in try ClientRulesFixtures.cases(fixture, "unread_label_cases") {
            let unread = try ClientRulesFixtures.int(testCase, "unread")
            let expected = try ClientRulesFixtures.string(testCase, "expect")
            XCTAssertEqual(WarrenForumActivity.label(unread: unread), expected)
        }
    }

    func testTheHeaderSlotCarriesWhatTheFixtureSaysForEveryPair() throws {
        for testCase in try ClientRulesFixtures.cases(fixture, "header_button_cases") {
            let button = WarrenForumActivity.headerButton(
                hasAccount: ClientRulesFixtures.bool(testCase, "has_account", or: false),
                enabled: ClientRulesFixtures.bool(testCase, "enabled", or: false))
            let expected = try ClientRulesFixtures.string(testCase, "expect")
            let name = try ClientRulesFixtures.string(testCase, "name")
            XCTAssertEqual(button.rawValue, expected, name)
        }
    }

    func testActivityIsShownOnlyForAnAccountWhoseOwnerLeftTheSettingOn() throws {
        for testCase in try ClientRulesFixtures.cases(fixture, "shows_activity_cases") {
            let expected = try ClientRulesFixtures.bool(testCase, "expect")
            XCTAssertEqual(
                WarrenForumActivity.showsActivity(
                    hasAccount: ClientRulesFixtures.bool(testCase, "has_account", or: false),
                    enabled: ClientRulesFixtures.bool(testCase, "enabled", or: false)),
                expected)
        }
    }

    func testTheWordingOfARiseFollowsTheFixture() throws {
        for testCase in try ClientRulesFixtures.cases(fixture, "wording_cases") {
            let expect = try ClientRulesFixtures.object(testCase, "expect")
            let wording: WarrenForumActivityWording =
                switch try ClientRulesFixtures.string(expect, "kind") {
                case "single": .single
                case "several": .several(count: try ClientRulesFixtures.int(expect, "count"))
                default: .moreThan(count: try ClientRulesFixtures.int(expect, "count"))
                }
            let unread = try ClientRulesFixtures.int(testCase, "unread")
            XCTAssertEqual(WarrenForumActivity.wording(unread: unread), wording)
        }
    }

    /// The storms the monitor exists to absorb, replayed step by step: what it
    /// publishes at the end, and every notification it raised on the way.
    func testTheMonitorAnswersEveryStormTheFixtureDescribes() throws {
        let cases = try ClientRulesFixtures.cases(fixture, "monitor_cases")
        XCTAssertGreaterThanOrEqual(cases.count, 9, "only \(cases.count) monitor cases reached this reader")
        for testCase in cases {
            let name = try ClientRulesFixtures.string(testCase, "name")
            let recorder = Recorder()
            let monitor = WarrenForumActivityMonitor(delegate: recorder)
            monitor.setEnabled(ClientRulesFixtures.bool(testCase, "enabled", or: true))
            monitor.setSlot((testCase["slot"] as? NSNumber)?.intValue)
            for step in try ClientRulesFixtures.cases(testCase, "steps") {
                try apply(step, to: monitor)
            }

            let expect = try ClientRulesFixtures.object(testCase, "expect")
            let published = try ClientRulesFixtures.int(expect, "unread")
            let notified = try ClientRulesFixtures.ints(expect, "notified")
            XCTAssertEqual(recorder.published, published, name)
            XCTAssertEqual(recorder.notified, notified, name)
            if expect["indicator"] != nil {
                XCTAssertEqual(
                    recorder.indicator, ClientRulesFixtures.bool(expect, "indicator", or: false), name)
            }
        }
    }

    private func apply(_ step: [String: Any], to monitor: WarrenForumActivityMonitor) throws {
        if step.keys.contains("digest") {
            monitor.setDigest(step["digest"] as? String)
        } else if let observed = step["observed"] as? NSNumber {
            monitor.setObservedUnread(observed.intValue)
        } else if step.keys.contains("slot") {
            monitor.setSlot((step["slot"] as? NSNumber)?.intValue)
        } else if let enabled = step["enabled"] as? Bool {
            monitor.setEnabled(enabled)
        } else {
            XCTFail("unknown monitor step \(step)")
        }
    }

    @MainActor
    private final class Recorder: WarrenForumActivityMonitor.Delegate {
        var notified: [Int] = []
        var published = 0
        var indicator = false

        func forumActivityDidRise(to unread: Int) { notified.append(unread) }
        func forumActivityShowsIndicator(_ showing: Bool) { indicator = showing }
        func forumActivityDidPublish(unread: Int) { published = unread }
    }
}
