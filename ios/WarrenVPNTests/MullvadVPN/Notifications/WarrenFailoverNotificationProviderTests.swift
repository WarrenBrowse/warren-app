//
//  WarrenFailoverNotificationProviderTests.swift
//  WarrenVPNTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import XCTest

@testable import WarrenVPN

private final class FakeFailoverAcknowledgeStore: WarrenFailoverAcknowledging {
    var warrenAcknowledgedFailoverCount: Int = 0
}

final class WarrenFailoverNotificationProviderTests: XCTestCase {
    func testShouldDisplayOnlyWhenCountExceedsAcknowledged() {
        XCTAssertTrue(WarrenFailoverNotificationProvider.shouldDisplay(failoverCount: 1, acknowledgedCount: 0))
        XCTAssertTrue(WarrenFailoverNotificationProvider.shouldDisplay(failoverCount: 5, acknowledgedCount: 2))
        XCTAssertFalse(WarrenFailoverNotificationProvider.shouldDisplay(failoverCount: 0, acknowledgedCount: 0))
        XCTAssertFalse(WarrenFailoverNotificationProvider.shouldDisplay(failoverCount: 2, acknowledgedCount: 2))
        // A stale acknowledged mark higher than the live count never displays.
        XCTAssertFalse(WarrenFailoverNotificationProvider.shouldDisplay(failoverCount: 1, acknowledgedCount: 3))
    }

    func testBannerShownWhenUnacknowledgedFailoverExists() {
        let store = FakeFailoverAcknowledgeStore()
        let provider = WarrenFailoverNotificationProvider(
            acknowledgeStore: store,
            failoverCountReader: { 2 }
        )

        let descriptor = provider.notificationDescriptor
        XCTAssertNotNil(descriptor)
        // Compare against the localized value (the test simulator may run in
        // any locale), which still proves the descriptor is rendered from the
        // intended "EXIT SWITCHED" key rather than an empty or wrong string.
        XCTAssertEqual(descriptor?.title, NSLocalizedString("EXIT SWITCHED", comment: ""))
        XCTAssertFalse(descriptor?.title.isEmpty ?? true)
        XCTAssertEqual(descriptor?.style, .warning)
    }

    func testBannerHiddenWhenAllFailoversAcknowledged() {
        let store = FakeFailoverAcknowledgeStore()
        store.warrenAcknowledgedFailoverCount = 4
        let provider = WarrenFailoverNotificationProvider(
            acknowledgeStore: store,
            failoverCountReader: { 4 }
        )

        XCTAssertNil(provider.notificationDescriptor)
    }

    func testDismissAcknowledgesCurrentCountAndHidesBanner() {
        let store = FakeFailoverAcknowledgeStore()
        var liveCount = 3
        let provider = WarrenFailoverNotificationProvider(
            acknowledgeStore: store,
            failoverCountReader: { liveCount }
        )

        let descriptor = provider.notificationDescriptor
        XCTAssertNotNil(descriptor)

        // Simulate the user tapping the close button on the banner.
        descriptor?.button?.handler?()

        XCTAssertEqual(store.warrenAcknowledgedFailoverCount, 3)
        XCTAssertNil(provider.notificationDescriptor)

        // A subsequent failover (count advances past the acknowledged mark)
        // re-arms the banner.
        liveCount = 4
        XCTAssertNotNil(provider.notificationDescriptor)
    }
}
