//
//  DeviceBannerRoutingTests.swift
//  WarrenVPNTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import Foundation
import PacketTunnelCore
import XCTest

@testable import WarrenVPN

/// The blocked-state banners that open the account's devices (warren-core doc
/// 115 section 7.1): the device limit and a session no token admits.
final class DeviceBannerRoutingTests: XCTestCase {
    func testTheDeviceLimitAndNoSessionTokenBannersOfferManageDevices() {
        XCTAssertEqual(TunnelStatusNotificationProvider.action(for: .tooManyDevices), .manageDevices)
        XCTAssertEqual(TunnelStatusNotificationProvider.action(for: .noSessionToken), .manageDevices)
    }

    func testOtherBlockedStatesKeepTheirOwnActionOrNone() {
        XCTAssertEqual(TunnelStatusNotificationProvider.action(for: .noRelaysSatisfyingPortConstraints), .showVPNSettings)
        XCTAssertNil(TunnelStatusNotificationProvider.action(for: .accountBanned))
        XCTAssertNil(TunnelStatusNotificationProvider.action(for: .invalidAccount))
    }

    func testManageDevicesOpensTheDeviceList() {
        XCTAssertEqual(
            ApplicationCoordinator.route(forTunnelStatusAction: TunnelStatusNotificationProvider.ActionIdentifier.manageDevices.rawValue),
            .devices
        )
        XCTAssertEqual(
            ApplicationCoordinator.route(forTunnelStatusAction: TunnelStatusNotificationProvider.ActionIdentifier.showVPNSettings.rawValue),
            .settings(.vpnSettings)
        )
    }

    /// The tap target has to be visible: the banner names it in its body.
    func testTheBannerBodyNamesManageDevices() {
        let body = TunnelStatusNotificationProvider.bodyText(for: .noSessionToken)

        XCTAssertEqual(
            body,
            "This device has no session credentials right now. Another device of your account may be holding them.\n**Manage devices**"
        )
        XCTAssertFalse(TunnelStatusNotificationProvider.bodyText(for: .accountBanned).contains("Manage devices"))
    }

    func testTheRevokedScreenSaysAnotherDeviceOfTheAccountDisconnectedIt() {
        XCTAssertEqual(
            RevokedDeviceView.bodyText,
            "This device was disconnected from another device of your account. Sign in with your recovery phrase to use it again."
        )
    }
}
