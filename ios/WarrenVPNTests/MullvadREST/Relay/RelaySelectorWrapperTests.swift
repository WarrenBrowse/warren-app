//
//  RelaySelectorWrapperTests.swift
//  MullvadVPNTests
//
//  Created by Jon Petersson on 2024-06-10.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import WarrenMockData
import XCTest
@testable import WarrenVPN

@testable import WarrenREST
@testable import WarrenSettings
@testable import WarrenTypes

class RelaySelectorWrapperTests: XCTestCase {
    let multihopWithDaitaConstraints = RelayConstraints(
        entryLocations: .only(UserSelectedRelays(locations: [.country("es")])),  // Relay with DAITA.
        exitLocations: .only(UserSelectedRelays(locations: [.country("us")]))
    )

    let multihopWithoutDaitaConstraints = RelayConstraints(
        entryLocations: .only(UserSelectedRelays(locations: [.country("se")])),  // Relay without DAITA.
        exitLocations: .only(UserSelectedRelays(locations: [.country("us")]))
    )

    let singlehopWithoutDaitaConstraints = RelayConstraints(
        exitLocations: .only(UserSelectedRelays(locations: [.country("se")]))  // Relay without DAITA.
    )

    let singlehopWithDaitaConstraints = RelayConstraints(
        exitLocations: .only(UserSelectedRelays(locations: [.country("es")]))  // Relay with DAITA.
    )

    var relayCache: RelayCache!
    override func setUpWithError() throws {
        let fileCache = MockFileCache(
            initialState: .exists(
                try StoredRelays(
                    rawData: try REST.Coding.makeJSONEncoder().encode(ServerRelaysResponseStubs.sampleRelays),
                    updatedAt: .distantPast
                ))
        )

        relayCache = RelayCache(fileCache: fileCache)
    }

    func testSelectRelayWithMultihopNever() throws {
        let wrapper = RelaySelectorWrapper(relayCache: relayCache)

        let settings = LatestTunnelSettings(
            relayConstraints: singlehopWithoutDaitaConstraints,
            tunnelMultihopState: .never,
            daita: DAITASettings(daitaState: .off)
        )

        let selectedRelays = try wrapper.selectRelays(tunnelSettings: settings, connectionAttemptCount: 0)
        XCTAssertNil(selectedRelays.entry)
    }

    func testSelectRelayWithMultihopAlways() throws {
        let wrapper = RelaySelectorWrapper(relayCache: relayCache)

        let settings = LatestTunnelSettings(
            relayConstraints: multihopWithDaitaConstraints,
            tunnelMultihopState: .always,
            daita: DAITASettings(daitaState: .off)
        )

        let selectedRelays = try wrapper.selectRelays(tunnelSettings: settings, connectionAttemptCount: 0)
        XCTAssertNotNil(selectedRelays.entry)
    }

    func testCanSelectRelayWithMultihopAlwaysAndDaitaOn() throws {
        let wrapper = RelaySelectorWrapper(relayCache: relayCache)

        let settings = LatestTunnelSettings(
            relayConstraints: multihopWithDaitaConstraints,
            tunnelMultihopState: .always,
            daita: DAITASettings(daitaState: .on)
        )

        XCTAssertNoThrow(try wrapper.selectRelays(tunnelSettings: settings, connectionAttemptCount: 0))
    }

    func testCannotSelectRelayWithMultihopAlwaysDaitaOn() throws {
        let wrapper = RelaySelectorWrapper(relayCache: relayCache)

        let settings = LatestTunnelSettings(
            relayConstraints: multihopWithoutDaitaConstraints,
            tunnelMultihopState: .always,
            daita: DAITASettings(daitaState: .on)
        )

        XCTAssertThrowsError(try wrapper.selectRelays(tunnelSettings: settings, connectionAttemptCount: 0))
    }

    func testCanSelectRelayWithMultihopNeverAndDaitaOn() throws {
        let wrapper = RelaySelectorWrapper(relayCache: relayCache)

        let settings = LatestTunnelSettings(
            relayConstraints: singlehopWithDaitaConstraints,
            tunnelMultihopState: .never,
            daita: DAITASettings(daitaState: .on)
        )

        let selectedRelays = try wrapper.selectRelays(tunnelSettings: settings, connectionAttemptCount: 0)
        XCTAssertNotNil(selectedRelays.exit)
    }

    // If DAITA is enabled and no supported relays are found, we should try to find the nearest
    // available relay that supports DAITA and use it as entry in a multihop selection.
    func testCanSelectRelayWithMultihopWhenNeededDaitaOnThroughMultihop() throws {
        let wrapper = RelaySelectorWrapper(relayCache: relayCache)

        let settings = LatestTunnelSettings(
            relayConstraints: singlehopWithoutDaitaConstraints,
            tunnelMultihopState: .whenNeeded,
            daita: DAITASettings(daitaState: .on)
        )

        let selectedRelays = try wrapper.selectRelays(tunnelSettings: settings, connectionAttemptCount: 0)
        XCTAssertNotNil(selectedRelays.entry)
    }

    func testValidWireguardPortDoesNotThrow() throws {
        let wrapper = RelaySelectorWrapper(relayCache: relayCache)

        let settings = LatestTunnelSettings(
            relayConstraints: .init(
                port:
                    .only(
                        ServerRelaysResponseStubs.sampleRelays.wireguard.portRanges.first!.first!
                    )
            )
        )

        XCTAssertNoThrow(
            try wrapper
                .selectRelays(tunnelSettings: settings, connectionAttemptCount: 0)
        )
    }

    func testInvalidWireguardPortThrows() throws {
        let wrapper = RelaySelectorWrapper(relayCache: relayCache)

        var settings = LatestTunnelSettings(
            relayConstraints: .init(port: .only(1)),
            wireGuardObfuscation: .init(state: .automatic)
        )

        XCTAssertThrowsError(
            try wrapper
                .selectRelays(tunnelSettings: settings, connectionAttemptCount: 0)
        )

        settings = LatestTunnelSettings(
            relayConstraints: .init(port: .only(1)),
            wireGuardObfuscation: .init(state: .off)
        )

        XCTAssertThrowsError(
            try wrapper
                .selectRelays(tunnelSettings: settings, connectionAttemptCount: 0)
        )
    }

    func testInvalidWireguardPortDoesNotThrowWhenObfuscated() throws {
        let wrapper = RelaySelectorWrapper(relayCache: relayCache)

        var settings = LatestTunnelSettings(
            relayConstraints: .init(port: .only(1)),
            wireGuardObfuscation: .init(state: .quic)
        )

        XCTAssertNoThrow(
            try wrapper
                .selectRelays(tunnelSettings: settings, connectionAttemptCount: 0)
        )

        settings = LatestTunnelSettings(
            relayConstraints: .init(port: .only(1)),
            wireGuardObfuscation: .init(state: .udpOverTcp)
        )

        XCTAssertNoThrow(
            try wrapper
                .selectRelays(tunnelSettings: settings, connectionAttemptCount: 0)
        )

        settings = LatestTunnelSettings(
            relayConstraints: .init(port: .only(1)),
            wireGuardObfuscation: .init(state: .shadowsocks)
        )

        XCTAssertNoThrow(
            try wrapper
                .selectRelays(tunnelSettings: settings, connectionAttemptCount: 0)
        )
    }

    /// Regression: FACTORY-DEFAULT settings must select a relay from the
    /// Warren fleet list, which serves no Swedish exit. The upstream
    /// Mullvad default pinned the exit location to `se`, so every fresh
    /// or reset install was blocked with `relayConstraintNotMatching` as
    /// soon as the fetched directory replaced the prebundled list
    /// (2026-07-17). The fixture mirrors the `/v1/exits` projection
    /// shape: Warren cities only, unified :443 port, DAITA-capable.
    func testDefaultSettingsSelectRelayFromWarrenFleet() throws {
        let warrenFleetJSON = """
            {
              "locations": {
                "de-berlin": {
                  "country": "Germany", "city": "Berlin",
                  "latitude": 52.52, "longitude": 13.405
                },
                "fi-helsinki": {
                  "country": "Finland", "city": "Helsinki",
                  "latitude": 64, "longitude": 26
                }
              },
              "wireguard": {
                "ipv4_gateway": "10.64.0.1",
                "ipv6_gateway": "fd00::1",
                "port_ranges": [[443, 443]],
                "shadowsocks_port_ranges": [],
                "relays": [
                  {
                    "hostname": "warren-aaaaaaaaaaaaaaaa",
                    "active": true, "owned": true, "provider": "warren",
                    "weight": 100,
                    "ipv4_addr_in": "198.51.100.1", "ipv6_addr_in": "::",
                    "public_key": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
                    "location": "de-berlin", "include_in_country": true,
                    "daita": true, "shadowsocks_extra_addr_in": [],
                    "features": null
                  },
                  {
                    "hostname": "warren-bbbbbbbbbbbbbbbb",
                    "active": true, "owned": true, "provider": "warren",
                    "weight": 100,
                    "ipv4_addr_in": "198.51.100.2", "ipv6_addr_in": "::",
                    "public_key": "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB=",
                    "location": "fi-helsinki", "include_in_country": true,
                    "daita": true, "shadowsocks_extra_addr_in": [],
                    "features": null
                  }
                ]
              },
              "bridge": { "shadowsocks": [], "relays": [] }
            }
            """
        let fileCache = MockFileCache(
            initialState: .exists(
                try StoredRelays(
                    rawData: Data(warrenFleetJSON.utf8),
                    updatedAt: .distantPast
                ))
        )
        let wrapper = RelaySelectorWrapper(relayCache: RelayCache(fileCache: fileCache))

        let selected = try wrapper.selectRelays(
            tunnelSettings: LatestTunnelSettings(),
            connectionAttemptCount: 0
        )
        XCTAssertNil(selected.entry)
        XCTAssertTrue(
            selected.exit.hostname.hasPrefix("warren-"),
            "Factory defaults must pick a Warren relay, got \(selected.exit.hostname)"
        )
    }
}
