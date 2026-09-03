//
//  WarrenAnnouncementNotificationProviderTests.swift
//  WarrenVPNTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import Foundation
import XCTest

@testable import WarrenRustRuntime
@testable import WarrenVPN

private final class FakeAnnouncementDismissStore: WarrenAnnouncementDismissing {
    var warrenDismissedAnnouncements: [String] = []
}

private func announcement(
    id: String = "a1",
    headline: String = "Production is open",
    body: String = "Your beta account gets a free month.",
    level: WarrenAnnouncementLevel = .warning,
    campaign: String? = "prod-launch",
    code: String? = nil
) -> WarrenAnnouncement {
    WarrenAnnouncement(
        id: id,
        headline: headline,
        body: body,
        level: level,
        cta: nil,
        voucherCampaignID: campaign,
        voucherCode: code
    )
}

final class WarrenAnnouncementNotificationProviderTests: XCTestCase {
    func testShouldDisplayTheFirstAnnouncementThatWasNotPutAway() {
        let first = announcement(id: "a1")
        let second = announcement(id: "a2")

        XCTAssertEqual(
            WarrenAnnouncementNotificationProvider.shouldDisplay(
                announcements: [first, second],
                dismissed: []
            )?.id,
            "a1",
            "the set arrives in publication order and the banner takes the first of it"
        )
        XCTAssertEqual(
            WarrenAnnouncementNotificationProvider.shouldDisplay(
                announcements: [first, second],
                dismissed: ["a1"]
            )?.id,
            "a2"
        )
        XCTAssertNil(
            WarrenAnnouncementNotificationProvider.shouldDisplay(
                announcements: [first, second],
                dismissed: ["a1", "a2"]
            )
        )
        XCTAssertNil(
            WarrenAnnouncementNotificationProvider.shouldDisplay(announcements: [], dismissed: [])
        )
    }

    func testDescriptorCarriesTheOperatorsHeadlineAndLeadsToTheFullText() {
        let provider = WarrenAnnouncementNotificationProvider(
            source: { [announcement()] },
            dismissStore: FakeAnnouncementDismissStore(),
            present: { _ in }
        )

        let descriptor = provider.notificationDescriptor

        XCTAssertNotNil(descriptor)
        XCTAssertEqual(descriptor?.identifier, .warrenAnnouncementInAppNotification)
        XCTAssertEqual(
            descriptor?.title,
            "Production is open",
            "the operator's own headline IS the title, rendered verbatim"
        )
        XCTAssertEqual(descriptor?.style, .warning)
        XCTAssertTrue(
            descriptor?.body.string.contains("Your beta account gets a free month.") ?? false
        )
        XCTAssertTrue(
            descriptor?.body.string.contains(
                NSLocalizedString("Read the full announcement.", comment: "")
            ) ?? false,
            "the banner is the compact entry; the code and the link live in the full view"
        )
    }

    func testTappingTheBannerOpensTheAnnouncementInFull() {
        var presented: WarrenAnnouncement?
        let provider = WarrenAnnouncementNotificationProvider(
            source: { [announcement(id: "a7")] },
            dismissStore: FakeAnnouncementDismissStore(),
            present: { presented = $0 }
        )

        provider.notificationDescriptor?.tapAction?.handler?()

        XCTAssertEqual(presented?.id, "a7")
    }

    func testDismissalIsByIdAndIsPermanent() {
        let store = FakeAnnouncementDismissStore()
        var visible = [announcement(id: "a1"), announcement(id: "a2")]
        let provider = WarrenAnnouncementNotificationProvider(
            source: { visible },
            dismissStore: store,
            present: { _ in }
        )

        provider.notificationDescriptor?.button?.handler?()

        XCTAssertEqual(store.warrenDismissedAnnouncements, ["a1"])
        XCTAssertEqual(
            provider.notificationDescriptor?.title,
            "Production is open",
            "the next announcement takes the slot; only the dismissed id is gone"
        )
        XCTAssertEqual(
            WarrenAnnouncementNotificationProvider.shouldDisplay(
                announcements: visible,
                dismissed: store.warrenDismissedAnnouncements
            )?.id,
            "a2"
        )

        // A re-publication of the same id stays put away, and the stored list
        // does not grow on a second dismissal of the same announcement.
        visible = [announcement(id: "a1")]
        provider.dismiss("a1")
        XCTAssertEqual(store.warrenDismissedAnnouncements, ["a1"])
        XCTAssertNil(provider.notificationDescriptor)
    }

    func testTheBannerLeadIsCutAtAWordBoundaryAndShortBodiesAreUntouched() {
        XCTAssertEqual(
            WarrenAnnouncementNotificationProvider.bannerLead("A short body.", limit: 140),
            "A short body.",
            "a body that fits reads exactly as the operator wrote it"
        )

        let long = String(repeating: "word ", count: 60)
        let lead = WarrenAnnouncementNotificationProvider.bannerLead(long, limit: 20)
        XCTAssertTrue(lead.hasSuffix("\u{2026}"))
        XCTAssertLessThanOrEqual(lead.count, 21)
        XCTAssertFalse(
            lead.dropLast().hasSuffix(" "),
            "the cut lands on a word boundary, with no dangling space before the ellipsis"
        )
    }

    func testTheInformationalLevelReadsAsTheCalmTierRatherThanAWarning() {
        // The banner has three tiers and the wire has an informational one.
        // Same mapping as the desktop card, so one announcement does not look
        // more urgent on one client than on another.
        XCTAssertEqual(WarrenAnnouncementNotificationProvider.style(for: .info), .success)
        XCTAssertEqual(WarrenAnnouncementNotificationProvider.style(for: .warning), .warning)
        XCTAssertEqual(WarrenAnnouncementNotificationProvider.style(for: .error), .error)
    }

    func testTheFullViewShowsNoCodeBlockWithoutACodeForThisAccount() {
        XCTAssertFalse(
            WarrenAnnouncementView.showsVoucherWell(announcement(campaign: nil)),
            "an announcement that carries no campaign has nothing to show a code for"
        )
        XCTAssertFalse(
            WarrenAnnouncementView.showsVoucherWell(announcement(campaign: "prod-launch")),
            "an account outside the cohort reads the operator's text without a code"
        )
        XCTAssertTrue(
            WarrenAnnouncementView.showsVoucherWell(
                announcement(campaign: "prod-launch", code: "ABCD1234EFGH5678")
            )
        )
    }
}
