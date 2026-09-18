//
//  WarrenForumDigestPollerTests.swift
//  WarrenVPNTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import WarrenRustRuntime
import XCTest

@testable import WarrenVPN

/// The cadence the foreground poll runs on, and what one fetch does to the
/// badge. Android covers the same decisions in `ForumDigestPollerTest`.
@MainActor
final class WarrenForumDigestPollerTests: XCTestCase {
    func testAServerThatAnsweredPutsTheLoopBackOnItsMinute() {
        for fetch: WarrenForumDigestFetch in [.ok, .notModified, .rejected] {
            let (delay, retry) = WarrenForumDigestCadence.next(
                unreachable: fetch.isUnreachable, retry: 40)
            XCTAssertEqual(delay, WarrenForumDigestCadence.checkInterval, fetch.rawValue)
            XCTAssertNil(retry, "an answer of any kind clears the fast retry")
        }
    }

    /// A client that just regained a network must not sit a full minute with a
    /// badge it can no longer justify, and must not hammer the host either.
    func testAFetchThatNeverReachedTheServerRetriesSoonAndBacksOffToItsCeiling() {
        var retry: TimeInterval?
        var delays: [TimeInterval] = []
        for _ in 0..<5 {
            let step = WarrenForumDigestCadence.next(unreachable: true, retry: retry)
            retry = step.retry
            delays.append(step.delay)
        }
        XCTAssertEqual(delays, [20, 40, 45, 45, 45])
    }

    func testAFetchIsWantedOnlyWhereSomethingReadsIt() {
        XCTAssertTrue(warrenForumDigestWanted(notificationsEnabled: true, hasAccount: true))
        XCTAssertFalse(warrenForumDigestWanted(notificationsEnabled: false, hasAccount: true))
        XCTAssertFalse(warrenForumDigestWanted(notificationsEnabled: true, hasAccount: false))
    }

    func testAVerifiedDocumentReachesTheBadge() async {
        var applied: [String?] = []
        let poller = WarrenForumDigestPoller(
            fetch: { WarrenForumDigest(counts: "03f", fetch: .ok) },
            preflight: { true },
            apply: { applied.append($0) })

        let unreachable = await poller.fetchOnce()

        XCTAssertEqual(applied, ["03f"])
        XCTAssertFalse(unreachable)
    }

    /// The tunnel is between states, so the API host name would go to a
    /// resolver that cannot answer. Nothing is asked, so nothing is learned,
    /// and the badge stands.
    func testATunnelBetweenStatesDefersTheFetchRatherThanHangingIt() async {
        // The fetch seam runs off the main actor, so what it records must be
        // reachable from both sides.
        let fetched = Flag()
        var applied = false
        let poller = WarrenForumDigestPoller(
            fetch: {
                fetched.raise()
                return WarrenForumDigest(counts: nil, fetch: .ok)
            },
            preflight: { false },
            apply: { _ in applied = true })

        let unreachable = await poller.fetchOnce()

        XCTAssertFalse(fetched.isRaised, "a deferred fetch must not reach the FFI")
        XCTAssertFalse(applied, "a deferred fetch says nothing about the badge")
        XCTAssertTrue(unreachable, "nothing was learned, so the loop retries soon")
    }

    /// An FFI that produced no envelope at all says nothing about the document
    /// Rust holds; reading that silence as "no activity" would clear a badge
    /// that is still justified.
    func testAnAbsentEnvelopeLeavesTheBadgeAlone() async {
        var applied = false
        let poller = WarrenForumDigestPoller(
            fetch: { nil },
            preflight: { true },
            apply: { _ in applied = true })

        let unreachable = await poller.fetchOnce()

        XCTAssertFalse(applied)
        XCTAssertTrue(unreachable)
    }

    /// One bit, readable from the fetch seam and from the test.
    private final class Flag: @unchecked Sendable {
        private let lock = NSLock()
        private var raised = false

        func raise() {
            lock.lock()
            defer { lock.unlock() }
            raised = true
        }

        var isRaised: Bool {
            lock.lock()
            defer { lock.unlock() }
            return raised
        }
    }

    /// Rust re-applies freshness on every read, so an expired document comes
    /// back as no counts and the badge drops by itself, whatever the fetch
    /// class was.
    func testAFetchThatFailedStillCarriesWhetherTheHeldDocumentIsStillFresh() async {
        var applied: [String?] = []
        let poller = WarrenForumDigestPoller(
            fetch: { WarrenForumDigest(counts: nil, fetch: .transport) },
            preflight: { true },
            apply: { applied.append($0) })

        let unreachable = await poller.fetchOnce()

        XCTAssertEqual(applied.count, 1)
        XCTAssertNil(applied[0])
        XCTAssertTrue(unreachable)
    }
}
