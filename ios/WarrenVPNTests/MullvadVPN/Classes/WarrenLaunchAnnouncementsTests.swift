//
//  WarrenLaunchAnnouncementsTests.swift
//  WarrenVPNTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import Foundation
import XCTest

@testable import WarrenRustRuntime
@testable import WarrenVPN

private let now = Date(timeIntervalSince1970: 1_800_000_000)

private func announcement(
    id: String = "a1",
    campaign: String? = nil
) -> WarrenAnnouncement {
    WarrenAnnouncement(
        id: id,
        headline: "Production is open",
        body: "Your beta account gets a free month.",
        level: .warning,
        cta: nil,
        voucherCampaignID: campaign
    )
}

final class WarrenLaunchAnnouncementsTests: XCTestCase {
    func testAFetchIsDueUntilOneLandsAndAgainAfterTheInterval() {
        XCTAssertTrue(warrenAnnouncementsFetchIsDue(lastFetch: nil, now: now, interval: 300))
        XCTAssertFalse(
            warrenAnnouncementsFetchIsDue(
                lastFetch: now.addingTimeInterval(-60),
                now: now,
                interval: 300
            )
        )
        XCTAssertTrue(
            warrenAnnouncementsFetchIsDue(
                lastFetch: now.addingTimeInterval(-300),
                now: now,
                interval: 300
            )
        )
        // A clock rollback counts as due rather than silencing the poll for an
        // arbitrary while.
        XCTAssertTrue(
            warrenAnnouncementsFetchIsDue(
                lastFetch: now.addingTimeInterval(3600),
                now: now,
                interval: 300
            )
        )
    }

    func testALapsedSetShowsNothingWithoutAFurtherFetch() {
        // The whole anti-freeze rule: a blocked or hostile network can suppress
        // a card (it could suppress the whole API anyway) but can never freeze
        // one on screen.
        let held = WarrenHeldAnnouncements(
            announcements: [announcement()],
            activeUntil: now.addingTimeInterval(60)
        )

        XCTAssertEqual(WarrenLaunchAnnouncements.displayable(held, now: now).count, 1)
        XCTAssertTrue(
            WarrenLaunchAnnouncements.displayable(held, now: now.addingTimeInterval(60)).isEmpty
        )
        XCTAssertTrue(
            WarrenLaunchAnnouncements.displayable(.empty, now: now).isEmpty,
            "nothing is displayed before an envelope has ever verified"
        )
    }

    func testAVerifiedFetchIsPublishedAndAnnouncedAsAChange() async {
        var changes = 0
        let live = WarrenVerifiedAnnouncements(
            announcements: [announcement(id: "a1")],
            activeUntil: Date().addingTimeInterval(3600)
        )
        let feed = makeFeed(verify: { _, _ in live })
        feed.didChange = { changes += 1 }

        await feed.refresh()

        XCTAssertEqual(feed.announcements.map(\.id), ["a1"])
        XCTAssertEqual(changes, 1)

        // The same set again is not a change: the banner is not re-rendered on
        // every poll of an unchanged announcement.
        await feed.refresh()
        XCTAssertEqual(changes, 1)
    }

    func testAnEnvelopeThatDidNotVerifyLeavesTheHeldSetAlone() async {
        var verified = true
        let live = WarrenVerifiedAnnouncements(
            announcements: [announcement(id: "a1")],
            activeUntil: Date().addingTimeInterval(3600)
        )
        let feed = makeFeed(verify: { _, _ in verified ? live : nil })
        await feed.refresh()
        XCTAssertEqual(feed.announcements.count, 1)

        verified = false
        await feed.refresh()

        XCTAssertEqual(
            feed.announcements.map(\.id),
            ["a1"],
            "a refusal must not erase an announcement the operator has not withdrawn"
        )
    }

    func testAFetchThatNeverGotAnAnswerLeavesTheHeldSetAlone() async {
        var reachable = true
        let live = WarrenVerifiedAnnouncements(
            announcements: [announcement(id: "a1")],
            activeUntil: Date().addingTimeInterval(3600)
        )
        let feed = makeFeed(
            fetch: { _ in reachable ? Data("{}".utf8) : nil },
            verify: { _, _ in live }
        )
        await feed.refresh()
        XCTAssertEqual(feed.announcements.count, 1)

        reachable = false
        await feed.refresh()

        XCTAssertEqual(feed.announcements.map(\.id), ["a1"])
    }

    func testTheWalletIsOnlyTouchedForAnAnnouncementThatCarriesAnOffer() async {
        var asked: [String] = []
        let live = WarrenVerifiedAnnouncements(
            announcements: [announcement(id: "plain", campaign: nil)],
            activeUntil: Date().addingTimeInterval(3600)
        )
        let feed = makeFeed(
            verify: { _, _ in live },
            voucher: { campaign in
                asked.append(campaign)
                return .drawn("ABCD1234EFGH5678")
            }
        )

        await feed.refresh()

        XCTAssertEqual(
            asked,
            [],
            "the signed lookup is the one request here tied to an account; it is never speculative"
        )
        XCTAssertNil(
            feed.announcements.first?.voucherCode,
            "no campaign, no code block"
        )
    }

    func testACampaignBearingAnnouncementCarriesThisAccountsCode() async {
        var asked: [String] = []
        let live = WarrenVerifiedAnnouncements(
            announcements: [announcement(id: "launch", campaign: "prod-launch")],
            activeUntil: Date().addingTimeInterval(3600)
        )
        let feed = makeFeed(
            verify: { _, _ in live },
            voucher: { campaign in
                asked.append(campaign)
                return .drawn("ABCD1234EFGH5678")
            }
        )

        await feed.refresh()

        XCTAssertEqual(asked, ["prod-launch"])
        XCTAssertEqual(feed.announcements.first?.voucherCode, "ABCD1234EFGH5678")
    }

    func testAnAccountOutsideTheCohortStillReadsTheAnnouncement() async {
        let live = WarrenVerifiedAnnouncements(
            announcements: [announcement(id: "launch", campaign: "prod-launch")],
            activeUntil: Date().addingTimeInterval(3600)
        )
        let feed = makeFeed(verify: { _, _ in live }, voucher: { _ in .outside })

        await feed.refresh()

        XCTAssertEqual(feed.announcements.count, 1)
        XCTAssertNil(feed.announcements.first?.voucherCode)
    }

    func testTheCodeIsDrawnOnceAndNotAskedForAgainOnEveryPoll() async {
        var asked: [String] = []
        let live = WarrenVerifiedAnnouncements(
            announcements: [announcement(id: "launch", campaign: "prod-launch")],
            activeUntil: Date().addingTimeInterval(3600)
        )
        let feed = makeFeed(
            verify: { _, _ in live },
            voucher: { campaign in
                asked.append(campaign)
                return .drawn("ABCD1234EFGH5678")
            }
        )

        await feed.refresh()
        await feed.refresh()
        await feed.refresh()

        // The lookup is wallet-signed. Re-issuing it on every five minute
        // refresh would turn a one-shot draw into a presence beacon: the
        // backend would learn roughly when this wallet has the app open, for
        // the life of the campaign.
        XCTAssertEqual(asked, ["prod-launch"])
        XCTAssertEqual(feed.announcements.first?.voucherCode, "ABCD1234EFGH5678")
    }

    func testAnAccountOutsideTheCohortIsNotAskedForAgain() async {
        var asked: [String] = []
        let live = WarrenVerifiedAnnouncements(
            announcements: [announcement(id: "launch", campaign: "prod-launch")],
            activeUntil: Date().addingTimeInterval(3600)
        )
        let feed = makeFeed(
            verify: { _, _ in live },
            voucher: { campaign in
                asked.append(campaign)
                return .outside
            }
        )

        await feed.refresh()
        await feed.refresh()

        // Outside the cohort is a final server answer, not a failure to retry.
        XCTAssertEqual(asked, ["prod-launch"])
        XCTAssertNil(feed.announcements.first?.voucherCode)
    }

    func testALookupThatNeverAnsweredIsAskedAgainOnTheNextPoll() async {
        var asked: [String] = []
        var reachable = false
        let live = WarrenVerifiedAnnouncements(
            announcements: [announcement(id: "launch", campaign: "prod-launch")],
            activeUntil: Date().addingTimeInterval(3600)
        )
        let feed = makeFeed(
            verify: { _, _ in live },
            voucher: { campaign in
                asked.append(campaign)
                return reachable ? .drawn("ABCD1234EFGH5678") : .unanswered
            }
        )

        await feed.refresh()
        XCTAssertNil(feed.announcements.first?.voucherCode)

        reachable = true
        await feed.refresh()

        // A transient outage must never tell a cohort member they were never
        // eligible, so nothing is held for it.
        XCTAssertEqual(asked, ["prod-launch", "prod-launch"])
        XCTAssertEqual(feed.announcements.first?.voucherCode, "ABCD1234EFGH5678")
    }

    func testAnAccountSwitchDropsTheCodeDrawnForThePreviousWallet() async {
        var asked: [String] = []
        var address = "wbAAAA"
        let live = WarrenVerifiedAnnouncements(
            announcements: [announcement(id: "launch", campaign: "prod-launch")],
            activeUntil: Date().addingTimeInterval(3600)
        )
        let feed = makeFeed(
            verify: { _, _ in live },
            voucher: { campaign in
                asked.append(campaign)
                return .drawn("ABCD1234EFGH5678")
            },
            address: { address }
        )

        await feed.refresh()
        XCTAssertEqual(feed.announcements.first?.voucherCode, "ABCD1234EFGH5678")

        address = "wbBBBB"
        await feed.refresh()

        // A code belongs to the wallet that asked for it, and the wallet that
        // replaces it has its own or none at all.
        XCTAssertEqual(asked, ["prod-launch", "prod-launch"])
    }

    func testACodeDrawnForAWalletReplacedMidRequestIsNeverShown() async {
        var address = "wbAAAA"
        let live = WarrenVerifiedAnnouncements(
            announcements: [announcement(id: "launch", campaign: "prod-launch")],
            activeUntil: Date().addingTimeInterval(3600)
        )
        let feed = makeFeed(
            verify: { _, _ in live },
            voucher: { _ in
                // The user restores another wallet while the signed request is
                // in flight.
                address = "wbBBBB"
                return .drawn("ABCD1234EFGH5678")
            },
            address: { address }
        )

        await feed.refresh()

        XCTAssertNil(
            feed.announcements.first?.voucherCode,
            "the previous wallet's code must never reach the new account's screen"
        )
    }

    func testADeviceHoldingNoWalletIsNeverAskedForACode() async {
        var asked: [String] = []
        let live = WarrenVerifiedAnnouncements(
            announcements: [announcement(id: "launch", campaign: "prod-launch")],
            activeUntil: Date().addingTimeInterval(3600)
        )
        let feed = makeFeed(
            verify: { _, _ in live },
            voucher: { campaign in
                asked.append(campaign)
                return .drawn("ABCD1234EFGH5678")
            },
            address: { nil }
        )

        await feed.refresh()

        XCTAssertEqual(asked, [])
        XCTAssertEqual(feed.announcements.count, 1, "the operator's text still reaches the reader")
        XCTAssertNil(feed.announcements.first?.voucherCode)
    }

    private func makeFeed(
        fetch: @escaping (URL) async -> Data? = { _ in Data("{}".utf8) },
        verify: @escaping (Data, String) -> WarrenVerifiedAnnouncements?,
        voucher: @escaping (String) async -> WarrenCampaignVoucherAnswer = { _ in .outside },
        address: @escaping () async -> String? = { "wbAAAA" }
    ) -> WarrenLaunchAnnouncements {
        WarrenLaunchAnnouncements(
            currentVersion: "2026.3",
            backend: WarrenLaunchAnnouncements.Backend(
                fetch: fetch,
                verify: verify,
                address: address,
                voucher: voucher
            )
        )
    }
}
