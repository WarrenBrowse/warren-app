//
//  WarrenForumPreflightTests.swift
//  WarrenVPNTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import Network
import WarrenREST
import WarrenTypes
import XCTest

@testable import WarrenVPN

/// The tunnel-state preflight replayed from
/// `fixtures/client-rules/forum_preflight.json`, the file the Android reader
/// replays on its side, plus the states only this platform has.
final class WarrenForumPreflightTests: XCTestCase {
    func testTheSharedPreflightFixtureReplaysCaseForCase() throws {
        let fixture = try ClientRulesFixtures.load("forum_preflight.json")
        let cases = try ClientRulesFixtures.cases(fixture, "cases").filter {
            !ClientRulesFixtures.skippedOnIOS($0)
        }
        XCTAssertGreaterThanOrEqual(cases.count, 7, "only \(cases.count) preflight cases reached this reader")
        for testCase in cases {
            let name = try ClientRulesFixtures.string(testCase, "name")
            let expect = try ClientRulesFixtures.object(testCase, "expect")
            let expected: ForumPreflight =
                switch try ClientRulesFixtures.string(expect, "verdict") {
                case "proceed": .proceed
                default: .deferred(tunnelClass: try ClientRulesFixtures.string(expect, "class"))
                }
            let state = try tunnel(named: try ClientRulesFixtures.string(testCase, "tunnel"))
            XCTAssertEqual(WarrenForumPreflight.verdict(for: state), expected, name)
        }
    }

    func testTheStatesOnlyThisPlatformHasAreClassedToo() {
        // An ephemeral-peer negotiation is the tail of a connect, and a
        // pending reconnect its head: both leave the resolver on the tunnel.
        XCTAssertEqual(
            WarrenForumPreflight.verdict(
                for: .negotiatingEphemeralPeer(
                    relays(), WireGuard.PrivateKey(), isPostQuantum: true, isDaita: false)),
            .deferred(tunnelClass: "connecting"))
        XCTAssertEqual(
            WarrenForumPreflight.verdict(for: .pendingReconnect),
            .deferred(tunnelClass: "reconnecting"))
        // The tunnel is installed and secured while its connection is down,
        // so a lookup still goes to its DNS and hangs.
        XCTAssertEqual(
            WarrenForumPreflight.verdict(for: .waitingForConnectivity(.noConnection)),
            .deferred(tunnelClass: "blocking"))
        // No network at all: nothing to wait for, and the transport failure
        // is what the person needs told.
        XCTAssertEqual(WarrenForumPreflight.verdict(for: .waitingForConnectivity(.noNetwork)), .proceed)
    }

    func testADeferralNamesTheClassAndNothingAboutTheRelay() throws {
        // The class reaches a log and a bug report, so it must never carry a
        // hostname, an endpoint or a blocked-state reason.
        let deferrals: [TunnelState] = [
            .connecting(relays(), isPostQuantum: false, isDaita: false),
            .reconnecting(relays(), isPostQuantum: false, isDaita: false),
            .error(.accountExpired),
        ]
        for state in deferrals {
            guard case let .deferred(tunnelClass) = WarrenForumPreflight.verdict(for: state) else {
                return XCTFail("\(state) must defer")
            }
            XCTAssertFalse(tunnelClass.contains("zz-exit"), tunnelClass)
            XCTAssertFalse(tunnelClass.contains("expired"), tunnelClass)
            XCTAssertTrue(
                ["connecting", "reconnecting", "disconnecting", "blocking"].contains(tunnelClass),
                tunnelClass)
        }
    }

    /// The fixture's platform-neutral state name as this client spells it.
    private func tunnel(named name: String) throws -> TunnelState {
        switch name {
        case "disconnected": .disconnected
        case "connected": .connected(relays(), isPostQuantum: false, isDaita: false)
        case "connecting": .connecting(relays(), isPostQuantum: false, isDaita: false)
        case "reconnecting": .reconnecting(relays(), isPostQuantum: false, isDaita: false)
        case "disconnecting": .disconnecting(.nothing)
        case "disconnecting_to_reconnect": .disconnecting(.reconnect)
        case "blocking": .error(.deviceRevoked)
        default: throw ClientRulesFixtures.Failure.missingKey(name)
        }
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
}
