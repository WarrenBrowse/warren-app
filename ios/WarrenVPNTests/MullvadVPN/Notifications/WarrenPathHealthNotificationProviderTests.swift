//
//  WarrenPathHealthNotificationProviderTests.swift
//  WarrenVPNTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import Network
import WarrenREST
import WarrenRustRuntime
import WarrenTypes
import XCTest

@testable import WarrenVPN

/// When the app says the exit has stopped forwarding.
///
/// The whole point of the verdict is the case every other guard misses: the
/// tunnel reports `connected`, the keep-alives answer, and nothing reaches the
/// internet. So the banner exists exactly in the state that otherwise reads as
/// "You are protected".
final class WarrenPathHealthNotificationProviderTests: XCTestCase {
    func testAWedgedDatapathSpeaksWhileTheTunnelStillClaimsToBeConnected() {
        XCTAssertEqual(
            WarrenPathHealthNotificationProvider.verdict(
                pathHealth: .degradedBoth,
                tunnelState: .connected(relays(), isPostQuantum: false, isDaita: false)),
            .degradedBoth)
    }

    func testASizeSelectiveBlackholeIsReportedApartFromADeadDatapath() {
        XCTAssertEqual(
            WarrenPathHealthNotificationProvider.verdict(
                pathHealth: .degradedLarge,
                tunnelState: .connected(relays(), isPostQuantum: false, isDaita: false)),
            .degradedLarge)
        XCTAssertNotEqual(
            WarrenPathHealthNotificationProvider.title(for: .degradedLarge),
            WarrenPathHealthNotificationProvider.title(for: .degradedBoth))
    }

    func testAHealthyDatapathSaysNothing() {
        XCTAssertNil(
            WarrenPathHealthNotificationProvider.verdict(
                pathHealth: .healthy,
                tunnelState: .connected(relays(), isPostQuantum: false, isDaita: false)))
    }

    /// In every other state the screen already tells the truth, and the
    /// verdict describes a session that may no longer be the live one.
    func testNoVerdictIsReportedOutsideAConnectedTunnel() {
        for state: TunnelState in [
            .disconnected, .connecting(relays(), isPostQuantum: false, isDaita: false),
            .reconnecting(relays(), isPostQuantum: false, isDaita: false), .pendingReconnect,
            .waitingForConnectivity(.noConnection), .disconnecting(.nothing),
        ] {
            XCTAssertNil(
                WarrenPathHealthNotificationProvider.verdict(
                    pathHealth: .degradedBoth, tunnelState: state),
                "\(state)")
        }
    }

    /// A verdict nobody published is not a degradation: an unset key means no
    /// session has measured anything yet.
    func testAnUnknownVerdictReadsAsHealthyRatherThanAsAFailure() {
        XCTAssertEqual(WarrenPathHealth(rawValue: 0), .healthy)
        XCTAssertNil(WarrenPathHealth(rawValue: 99))
    }

    private func relays() -> SelectedRelays {
        SelectedRelays(entry: nil, exit: relay(), retryAttempt: 0)
    }

    private func relay() -> SelectedRelay {
        SelectedRelay(
            endpoint: SelectedEndpoint(
                socketAddress: .ipv4(IPv4Endpoint(ip: .loopback, port: 1300)),
                ipv4Gateway: .loopback,
                ipv6Gateway: .loopback,
                publicKey: WireGuard.PrivateKey().publicKey.rawValue,
                obfuscation: .off
            ),
            hostname: "zz-exit-1",
            location: Location(
                country: "Testland",
                countryCode: "zz",
                city: "Testcity",
                cityCode: "zz1",
                latitude: 0,
                longitude: 0
            ),
            isIPOverridden: false,
            features: nil
        )
    }

    func testEveryDegradedVerdictCarriesATitleAndABody() {
        for health: WarrenPathHealth in [.degradedBoth, .degradedLarge] {
            XCTAssertFalse(
                WarrenPathHealthNotificationProvider.title(for: health).isEmpty, "\(health)")
            XCTAssertFalse(
                WarrenPathHealthNotificationProvider.body(for: health).isEmpty, "\(health)")
        }
    }
}
