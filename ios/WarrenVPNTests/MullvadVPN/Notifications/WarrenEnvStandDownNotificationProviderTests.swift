//
//  WarrenEnvStandDownNotificationProviderTests.swift
//  WarrenVPNTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import Foundation
import XCTest

@testable import WarrenSettings
@testable import WarrenVPN

private final class FakeStandDownRecordStore: WarrenEnvStandDownStoring {
    var warrenEnvStandDown = WarrenEnvStandDownRecord()
}

final class WarrenEnvStandDownNotificationProviderTests: XCTestCase {
    /// The offer, and the stand-down it may lead to, are both the banner. Only
    /// an answered record takes it away.
    func testShouldDisplayWhileAHigherEnvironmentIsOfferedOrStoodDownFor() {
        XCTAssertFalse(
            WarrenEnvStandDownNotificationProvider.shouldDisplay(record: WarrenEnvStandDownRecord())
        )
        XCTAssertTrue(
            WarrenEnvStandDownNotificationProvider.shouldDisplay(
                record: WarrenEnvStandDownRecord(higherEnvironmentSeen: "prod")
            )
        )
        XCTAssertTrue(
            WarrenEnvStandDownNotificationProvider.shouldDisplay(
                record: WarrenEnvStandDownRecord(
                    higherEnvironmentSeen: "prod",
                    standDownApplied: true
                )
            )
        )
        // The user asked for this build back, or turned the offer down: the
        // banner goes and stays gone, even though the other install is still
        // there.
        XCTAssertFalse(
            WarrenEnvStandDownNotificationProvider.shouldDisplay(
                record: WarrenEnvStandDownRecord(higherEnvironmentSeen: "prod", reEnabled: true)
            )
        )
    }

    /// Nothing has been given up yet, so the banner asks rather than reports,
    /// and the tap is the consent the teardown needs.
    func testTheOfferAsksBeforeAnythingIsGivenUp() {
        let store = FakeStandDownRecordStore()
        store.warrenEnvStandDown = WarrenEnvStandDownRecord(higherEnvironmentSeen: "prod")
        var confirmed = false
        var reEnabled = false
        let provider = WarrenEnvStandDownNotificationProvider(
            store: store,
            confirm: { confirmed = true },
            reEnable: { reEnabled = true }
        )

        let descriptor = provider.notificationDescriptor

        XCTAssertNotNil(descriptor)
        XCTAssertEqual(descriptor?.identifier, .warrenEnvStandDownInAppNotification)
        XCTAssertEqual(descriptor?.style, .warning)
        XCTAssertFalse(descriptor?.body.string.isEmpty ?? true)

        // The close button turns the offer down: the signal behind it cannot
        // be authenticated, so putting it away must cost nothing.
        descriptor?.button?.handler?()
        XCTAssertTrue(reEnabled)
        XCTAssertFalse(confirmed)

        descriptor?.tapAction?.handler?()
        XCTAssertTrue(confirmed)
    }

    func testDescriptorNamesTheStandDownAndOffersTheReEnable() {
        let store = FakeStandDownRecordStore()
        store.warrenEnvStandDown = WarrenEnvStandDownRecord(
            higherEnvironmentSeen: "prod",
            standDownApplied: true
        )
        var reEnabled = false
        let provider = WarrenEnvStandDownNotificationProvider(
            store: store,
            confirm: {},
            reEnable: { reEnabled = true }
        )

        let descriptor = provider.notificationDescriptor

        XCTAssertNotNil(descriptor)
        XCTAssertEqual(descriptor?.identifier, .warrenEnvStandDownInAppNotification)
        XCTAssertEqual(descriptor?.style, .warning)
        // Compare against the localized value (the simulator may run in any
        // locale), which still proves the descriptor is rendered from the
        // intended key rather than an empty or wrong string.
        XCTAssertEqual(descriptor?.title, NSLocalizedString("PRODUCTION HAS PRIORITY", comment: ""))
        XCTAssertFalse(descriptor?.title.isEmpty ?? true)
        XCTAssertFalse(descriptor?.body.string.isEmpty ?? true)

        descriptor?.tapAction?.handler?()

        XCTAssertTrue(reEnabled)
    }

    func testBannerHiddenOnceTheUserReEnabledThisBuild() {
        let store = FakeStandDownRecordStore()
        store.warrenEnvStandDown = WarrenEnvStandDownRecord(
            higherEnvironmentSeen: "prod",
            reEnabled: true,
            standDownApplied: true
        )
        let provider = WarrenEnvStandDownNotificationProvider(
            store: store,
            confirm: {},
            reEnable: {}
        )

        XCTAssertNil(provider.notificationDescriptor)
    }

    /// Only the highest-priority descriptor reaches the banner, and while this
    /// build has stood down every tunnel message describes a tunnel it no
    /// longer holds. `TunnelStatusNotificationProvider` is the `.critical` one
    /// this has to outrank.
    func testItOutranksTheTunnelStatusBanner() {
        let provider = WarrenEnvStandDownNotificationProvider(
            store: FakeStandDownRecordStore(),
            confirm: {},
            reEnable: {}
        )

        XCTAssertGreaterThan(provider.priority, .critical)
    }
}
