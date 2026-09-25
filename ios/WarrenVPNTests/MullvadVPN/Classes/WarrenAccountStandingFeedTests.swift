//
//  WarrenAccountStandingFeedTests.swift
//  WarrenVPNTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import Foundation
import XCTest

@testable import WarrenRustRuntime
@testable import WarrenVPN

private let now = Date(timeIntervalSince1970: 1_800_000_000)

private func strike(_ reference: String) -> WarrenAccountStrike {
    WarrenAccountStrike(
        day: Date(timeIntervalSince1970: 1_790_208_000),
        category: "copyright",
        exitCountry: "FI",
        port: 51413,
        caseReference: reference
    )
}

private func standing(_ references: String...) -> WarrenAccountStanding {
    WarrenAccountStanding(strikes: references.map(strike), threshold: 3, windowDays: 90, ban: nil)
}

/// Records what the feed asked of the outside world.
private final class Recorder: @unchecked Sendable {
    var polls: [WarrenStandingPoll?]
    var hasWallet = true
    var announced: [WarrenStrikeNotice] = []
    var forgets = 0

    init(polls: [WarrenStandingPoll?]) {
        self.polls = polls
    }

    var backend: WarrenAccountStandingFeed.Backend {
        WarrenAccountStandingFeed.Backend(
            hasWallet: { self.hasWallet },
            poll: { self.polls.isEmpty ? nil : self.polls.removeFirst() },
            forget: { self.forgets += 1 },
            announce: { self.announced.append($0) }
        )
    }
}

private final class DismissStore: WarrenStrikeDismissing {
    var warrenDismissedStrikes: [String] = []
}

final class WarrenAccountStandingFeedTests: XCTestCase {
    func testEachNewStrikeIsAnnouncedOnceAndTheSameStandingAgainAnnouncesNothing() async {
        let first = WarrenStandingPoll(
            ok: true, reported: true, standing: standing("PF-1"),
            newStrikes: [WarrenStrikeNotice(strike: strike("PF-1"), ordinal: 1, threshold: 3)]
        )
        let again = WarrenStandingPoll(ok: true, reported: true, standing: standing("PF-1"), newStrikes: [])
        let recorder = Recorder(polls: [first, again])
        let feed = WarrenAccountStandingFeed(backend: recorder.backend)

        await feed.refresh(now: now)
        await feed.refresh(now: now)

        XCTAssertEqual(recorder.announced.map(\.strike.caseReference), ["PF-1"])
        XCTAssertEqual(feed.standing, standing("PF-1"))
    }

    func testAFailedPollKeepsWhatIsShown() async {
        let shown = WarrenStandingPoll(ok: true, reported: true, standing: standing("PF-1"), newStrikes: [])
        let failed = WarrenStandingPoll(ok: false, reported: true, standing: nil, newStrikes: [])
        let recorder = Recorder(polls: [shown, failed])
        let feed = WarrenAccountStandingFeed(backend: recorder.backend)

        await feed.refresh(now: now)
        await feed.refresh(now: now)

        XCTAssertEqual(feed.standing, standing("PF-1"))
    }

    func testAPollYoungerThanTheCadenceIsNotRepeated() async {
        let answer = WarrenStandingPoll(ok: true, reported: true, standing: nil, newStrikes: [])
        let recorder = Recorder(polls: [answer, answer])
        let feed = WarrenAccountStandingFeed(backend: recorder.backend)

        await feed.refreshIfDue(now: now)
        await feed.refreshIfDue(now: now.addingTimeInterval(9 * 60))

        XCTAssertEqual(recorder.polls.count, 1, "the second poll came before the cadence")
    }

    func testAWalletThatLeftTakesItsStandingWithIt() async {
        let shown = WarrenStandingPoll(ok: true, reported: true, standing: standing("PF-1"), newStrikes: [])
        let recorder = Recorder(polls: [shown])
        let feed = WarrenAccountStandingFeed(backend: recorder.backend)
        await feed.refresh(now: now)

        recorder.hasWallet = false
        await feed.refresh(now: now)

        XCTAssertNil(feed.standing)
        XCTAssertEqual(recorder.forgets, 1)
    }

    func testTheBannerShowsTheNewestStrikeUntilItIsPutAway() {
        let store = DismissStore()

        let shown = WarrenAccountStrikeNotificationProvider.shouldDisplay(
            standing: standing("PF-1", "PF-2"),
            dismissed: store.warrenDismissedStrikes
        )
        XCTAssertEqual(shown?.strike.caseReference, "PF-2")
        XCTAssertEqual(shown?.ordinal, 2)

        XCTAssertNil(
            WarrenAccountStrikeNotificationProvider.shouldDisplay(
                standing: standing("PF-1", "PF-2"),
                dismissed: [strike("PF-2").dismissalKey]
            )
        )
    }

    func testAStrikeDayIsTheUTCDayItWasRecordedOn() {
        let day = WarrenAccountStandingText.day(
            Date(timeIntervalSince1970: 1_790_208_000),
            locale: Locale(identifier: "en_US")
        )

        XCTAssertEqual(day, "September 24, 2026")
    }
}
