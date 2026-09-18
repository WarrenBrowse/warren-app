//
//  WarrenConnectingStuckNotificationProviderTests.swift
//  WarrenVPNTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import Network
import WarrenREST
import WarrenTypes
import XCTest

@testable import WarrenVPN

/// Which tunnel states count as "still trying to connect".
///
/// This is the whole decision behind the "TROUBLE CONNECTING?" banner, which
/// iOS did not have at all: a stalled connect spun with no hint and no way to
/// report it. The collapse to one bool is what makes the 45 second timer
/// usable, because an attempt walks through several states while it redials
/// and re-arming on each of them would push the banner permanently out of
/// reach. Android makes the same collapse in
/// `ConnectingStuckNotificationUseCase` and says so for the same reason.
final class WarrenConnectingStuckNotificationProviderTests: XCTestCase {
    private func isConnecting(_ state: TunnelState) -> Bool {
        WarrenConnectingStuckNotificationProvider.isConnectingPhase(state)
    }

    private func relays() -> SelectedRelays {
        SelectedRelays(
            entry: nil,
            exit: SelectedRelay(
                endpoint: SelectedEndpoint(
                    socketAddress: .ipv4(IPv4Endpoint(ip: .loopback, port: 1300)),
                    ipv4Gateway: .loopback,
                    ipv6Gateway: .loopback,
                    publicKey: WireGuard.PrivateKey().publicKey.rawValue,
                    obfuscation: .off
                ),
                hostname: "zz-exit-1",
                location: Location(
                    country: "Testland", countryCode: "zz",
                    city: "Testcity", cityCode: "zz1",
                    latitude: 0, longitude: 0
                ),
                isIPOverridden: false,
                features: nil
            ),
            retryAttempt: 0
        )
    }

    func testEveryStateOfAnAttemptComingUpCounts() {
        XCTAssertTrue(isConnecting(.connecting(nil, isPostQuantum: false, isDaita: false)))
        XCTAssertTrue(isConnecting(.reconnecting(relays(), isPostQuantum: false, isDaita: false)))
        XCTAssertTrue(
            isConnecting(
                .negotiatingEphemeralPeer(
                    relays(), WireGuard.PrivateKey(), isPostQuantum: false, isDaita: false)))
        XCTAssertTrue(isConnecting(.pendingReconnect))
    }

    /// The difference that matters: a teardown on the way to another dial is
    /// still the same attempt, a teardown on the way out is not.
    func testATeardownCountsOnlyWhenADialFollowsIt() {
        XCTAssertTrue(isConnecting(.disconnecting(.reconnect)))
        XCTAssertFalse(isConnecting(.disconnecting(.nothing)))
    }

    func testASettledOrBlockedTunnelDoesNotCount() {
        XCTAssertFalse(isConnecting(.disconnected))
        XCTAssertFalse(isConnecting(.waitingForConnectivity(.noConnection)))
        XCTAssertFalse(isConnecting(.waitingForConnectivity(.noNetwork)))
        XCTAssertFalse(isConnecting(.error(.noRelaysSatisfyingConstraints)))
    }

    /// The window is a shared number, not this client's opinion: desktop's
    /// `useConnectingStuck` and Android's `STUCK_WINDOW` are both 45 seconds.
    func testTheWindowIsTheOneTheOtherClientsWaitFor() {
        XCTAssertEqual(WarrenConnectingStuckNotificationProvider.stuckAfter, 45)
    }
}
