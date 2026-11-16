//
//  HomeStatusTests.swift
//  WarrenVPNTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import Network
import PacketTunnelCore
import WarrenREST
import WarrenTypes
import XCTest

@testable import WarrenVPN

/// The home status block says what desktop's `home-status-text.ts` says, phase by phase. The test
/// plan runs in English, so the expectations are the source strings.
final class HomeStatusTests: XCTestCase {
    static func relay(hostname: String, country: String, code: String, city: String) -> SelectedRelay {
        SelectedRelay(
            endpoint: SelectedEndpoint(
                socketAddress: .ipv4(IPv4Endpoint(ip: .loopback, port: 1300)),
                ipv4Gateway: .loopback,
                ipv6Gateway: .loopback,
                publicKey: WireGuard.PrivateKey().publicKey.rawValue,
                obfuscation: .off
            ),
            hostname: hostname,
            location: Location(
                country: country,
                countryCode: code,
                city: city,
                cityCode: "zz1",
                latitude: 0,
                longitude: 0
            ),
            isIPOverridden: false,
            features: nil
        )
    }

    static func helsinki(entry: SelectedRelay? = nil) -> SelectedRelays {
        SelectedRelays(
            entry: entry,
            exit: relay(hostname: "warren-3cb774609e5b6876", country: "Finland", code: "fi", city: "Helsinki"),
            retryAttempt: 0
        )
    }

    static let singapore = SelectedRelays(
        entry: nil,
        exit: relay(hostname: "warren-sg", country: "Singapore", code: "sg", city: "Singapore"),
        retryAttempt: 0
    )

    func testExposedSaysTheUserIsVisible() {
        XCTAssertEqual(
            HomeStatus(tunnelState: .disconnected),
            HomeStatus(pill: "Unprotected", title: "You are visible", subtitle: "Your connection is not encrypted")
        )
    }

    func testProtectedNamesThePlaceCityFirstWithTheHostInMono() {
        let status = HomeStatus(tunnelState: .connected(Self.helsinki(), isPostQuantum: false, isDaita: false))
        XCTAssertEqual(
            status,
            HomeStatus(
                pill: "Protected", title: "Helsinki, Finland", subtitle: "warren-3cb774609e5b6876",
                subtitleIsHostname: true)
        )
    }

    func testATwoHopCircuitNamesTheEntryTheExitIsReachedThrough() {
        let entry = Self.relay(hostname: "warren-entry", country: "Sweden", code: "se", city: "Stockholm")
        let status = HomeStatus(
            tunnelState: .connected(Self.helsinki(entry: entry), isPostQuantum: false, isDaita: false))
        XCTAssertEqual(status.subtitle, "warren-3cb774609e5b6876 via warren-entry")
        XCTAssertTrue(status.subtitleIsHostname)
    }

    func testACityStateIsNamedOnce() {
        let status = HomeStatus(tunnelState: .connected(Self.singapore, isPostQuantum: false, isDaita: false))
        XCTAssertEqual(status.title, "Singapore")
    }

    func testConnectingNamesTheTargetAndSaysTheConnectionIsInProgress() {
        XCTAssertEqual(
            HomeStatus(tunnelState: .connecting(Self.helsinki(), isPostQuantum: false, isDaita: false)),
            HomeStatus(pill: "Connecting", title: "Helsinki, Finland", subtitle: "Connection in progress")
        )
        XCTAssertEqual(
            HomeStatus(tunnelState: .reconnecting(Self.helsinki(), isPostQuantum: false, isDaita: false)).title,
            "Helsinki, Finland"
        )
    }

    func testConnectingWithNoTargetYetSaysOnlyThatItIsInProgress() {
        XCTAssertEqual(
            HomeStatus(tunnelState: .connecting(nil, isPostQuantum: false, isDaita: false)),
            HomeStatus(pill: "Connecting", title: "Connection in progress", subtitle: "")
        )
    }

    func testDisconnectingIsVisibleAgainOnTheWayDown() {
        XCTAssertEqual(
            HomeStatus(tunnelState: .disconnecting(.nothing)),
            HomeStatus(pill: "Connecting", title: "You are visible", subtitle: "Disconnecting...")
        )
    }

    func testInterruptedStaysProtected() {
        XCTAssertEqual(
            HomeStatus(tunnelState: .waitingForConnectivity(.noNetwork)),
            HomeStatus(pill: "Interrupted", title: "Connection interrupted", subtitle: "You are protected")
        )
    }

    func testTheKillSwitchSaysTheConnectionIsBlocked() {
        let blocked = HomeStatus(pill: "Blocked", title: "Connection blocked", subtitle: "")
        XCTAssertEqual(HomeStatus(tunnelState: .waitingForConnectivity(.noConnection)), blocked)
        XCTAssertEqual(
            HomeStatus(tunnelState: .error(.noRelaysSatisfyingConstraints)), blocked)
    }
}
