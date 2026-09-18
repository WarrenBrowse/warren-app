//
//  WarrenForumActivityPanelTests.swift
//  WarrenVPNTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import WarrenRustRuntime
import XCTest

@testable import WarrenVPN

/// What opening the forum activity panel does, and what each row says.
@MainActor
final class WarrenForumActivityPanelTests: XCTestCase {
    private func notification(
        id: Int64 = 1,
        kind: WarrenForumNotificationKind = .replied,
        createdAt: Date = Date(timeIntervalSince1970: 1_800_000_000),
        actor: String? = "lusab-babad-dovok",
        path: String? = "/t/86/4"
    ) -> WarrenForumNotification {
        WarrenForumNotification(
            id: id,
            kind: kind,
            unread: true,
            createdAt: createdAt,
            title: "A topic",
            actor: actor,
            excerpt: nil,
            path: path
        )
    }

    private struct Reader: WarrenForumActivityReading {
        let result: WarrenForumNotificationsResult
        let seen: WarrenBox<Bool>

        func list() async -> WarrenForumNotificationsResult { result }

        func markSeen() async -> Bool {
            seen.value = true
            return true
        }
    }

    /// One digest character per slot, so the panel's own read is the only
    /// exact count: the rows arrive newest first and the badge follows at
    /// once, rather than waiting for the digest to catch up.
    func testOpeningThePanelShowsTheRowsNewestFirstAndClearsTheBadge() async {
        let older = notification(id: 1, createdAt: Date(timeIntervalSince1970: 1_000))
        let newer = notification(id: 2, createdAt: Date(timeIntervalSince1970: 2_000))
        let seen = WarrenBox(false)
        var observed: [Int] = []
        let viewModel = WarrenForumActivityViewModel(
            reader: Reader(result: .ok([older, newer]), seen: seen),
            observe: { observed.append($0) })

        await viewModel.load()

        XCTAssertEqual(viewModel.state.notifications.map(\.id), [2, 1])
        XCTAssertEqual(observed, [0], "what the reader is looking at is read")
        XCTAssertTrue(seen.value, "opening the panel marks the list seen on the forum")
    }

    /// A failed read proves nothing about the count, so the badge is left
    /// exactly as it stands, and nothing is marked seen.
    func testAFailedReadLeavesTheBadgeAloneAndMarksNothing() async {
        let seen = WarrenBox(false)
        var observed: [Int] = []
        let viewModel = WarrenForumActivityViewModel(
            reader: Reader(result: .failed(reason: "transport"), seen: seen),
            observe: { observed.append($0) })

        await viewModel.load()

        XCTAssertEqual(viewModel.state, .failed(reason: "transport"))
        XCTAssertTrue(observed.isEmpty)
        XCTAssertFalse(seen.value)
    }

    func testEveryKindSaysWhoDidWhatAndNeverFallsThroughToNothing() {
        for kind in [
            WarrenForumNotificationKind.mentioned, .replied, .quoted, .liked, .privateMessage,
            .posted, .linked, .grantedBadge, .watchingFirstPost, .announcement, .other,
        ] {
            let headline = WarrenForumActivityRow.headline(for: notification(kind: kind))
            XCTAssertFalse(headline.isEmpty, kind.rawValue)
            XCTAssertFalse(
                WarrenForumActivityRow.symbol(for: kind).isEmpty, kind.rawValue)
        }
    }

    /// A row the forum did not attribute still reads as a sentence rather
    /// than starting with an empty name.
    func testARowWithNoNamedMemberStillReadsAsASentence() {
        let headline = WarrenForumActivityRow.headline(
            for: notification(kind: .liked, actor: nil))
        XCTAssertFalse(headline.hasPrefix(" "))
        XCTAssertFalse(headline.isEmpty)
    }

    /// Past a week "5 weeks ago" tells a reader less than the day it
    /// happened, which is the threshold the badge rules pin.
    func testAnAgeReadsAsADateOnceRelativeStopsHelping() {
        let now = Date(timeIntervalSince1970: 1_800_000_000)
        let english = Locale(identifier: "en_US")

        let recent = WarrenForumActivityRow.age(
            of: now.addingTimeInterval(-3600), now: now, locale: english)
        let old = WarrenForumActivityRow.age(
            of: now.addingTimeInterval(-30 * 24 * 3600), now: now, locale: english)

        XCTAssertNotEqual(recent, old)
        XCTAssertTrue(old.contains(where: \.isNumber), "a date carries its day")
        XCTAssertFalse(
            WarrenForumActivity.ageIsRelative(
                createdAt: now.addingTimeInterval(-30 * 24 * 3600), now: now))
    }

    /// The path was held to the shapes the forum produces in Rust, so a row
    /// can only ever open inside the forum origin, and a row that points at
    /// nothing openable opens nothing.
    func testARowOpensInsideTheForumOriginOrNothingAtAll() throws {
        let url = try XCTUnwrap(WarrenForumActivityRow.url(for: notification()))
        XCTAssertTrue(
            url.absoluteString.hasPrefix(WarrenProductAnchors.current.forumPublicURL),
            url.absoluteString)
        XCTAssertNil(WarrenForumActivityRow.url(for: notification(path: nil)))
    }

    func testTheAlertSaysWhatTheCountAllowsItToSay() {
        XCTAssertFalse(WarrenForumActivityAlert.message(unread: 1).isEmpty)
        XCTAssertTrue(WarrenForumActivityAlert.message(unread: 4).contains("4"))
        // Saying "15" would be a number the user can check and find wrong.
        XCTAssertTrue(WarrenForumActivityAlert.message(unread: 15).contains("14"))
    }
}

/// A reference cell, so a value written inside a `Sendable` reader is
/// readable from the test.
final class WarrenBox<Value>: @unchecked Sendable {
    private let lock = NSLock()
    private var stored: Value

    init(_ value: Value) {
        stored = value
    }

    var value: Value {
        get {
            lock.lock()
            defer { lock.unlock() }
            return stored
        }
        set {
            lock.lock()
            defer { lock.unlock() }
            stored = newValue
        }
    }
}
