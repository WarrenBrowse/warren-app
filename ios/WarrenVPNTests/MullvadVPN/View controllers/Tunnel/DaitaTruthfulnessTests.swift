//
//  DaitaTruthfulnessTests.swift
//  WarrenVPNTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import Network
import WarrenMockData
import WarrenREST
import WarrenSettings
import WarrenTypes
import XCTest

@testable import WarrenVPN

/// The connect screen showed a plain "DAITA" chip on every connection made with
/// the DAITA toggle on, and `TunnelState.isDaita` is that toggle plumbed
/// through, not a grant from the exit. The iOS datapath negotiates no DAITA at
/// all: `warren_tunnel_ffi.rs` dials with the defense off and says so in a
/// comment. So the chip asserted a protection that was not on the wire.
///
/// The desktop daemon reads the exit's own echo before claiming anything
/// (`talpid-warren-tunnel/src/lib.rs`: "the Connected state never claims a
/// protection that is not running"). Until iOS carries that echo, its chip says
/// the defense is not active, which is true and is the wording the other two
/// clients already use.
///
/// When iOS does start negotiating DAITA, this test is the thing that fails and
/// points at the chip.
final class DaitaTruthfulnessTests: XCTestCase {
    private func connected(isDaita: Bool) -> TunnelState {
        let exit = SelectedRelay(
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
        )
        return .connected(
            SelectedRelays(entry: nil, exit: exit, retryAttempt: 0),
            isPostQuantum: false,
            isDaita: isDaita
        )
    }

    private func settings(daita: Bool) -> LatestTunnelSettings {
        var settings = LatestTunnelSettings()
        settings.daita = DAITASettings(daitaState: daita ? .on : .off)
        return settings
    }

    func testTheChipNeverClaimsDaitaIsRunning() {
        let feature = DaitaFeature(
            state: connected(isDaita: true),
            settings: settings(daita: true)
        )
        XCTAssertEqual(feature.name, "DAITA: not active on this server")
        XCTAssertFalse(
            feature.name == "DAITA",
            "the chip claims a defense this client does not negotiate")
    }

    /// The chip exists to tell the user the thing they asked for is not
    /// happening, so it is shown exactly when they asked for it.
    func testTheChipIsShownWhenTheUserAskedForDaitaAndNotOtherwise() {
        let state = connected(isDaita: false)
        XCTAssertTrue(DaitaFeature(state: state, settings: settings(daita: true)).isEnabled)
        XCTAssertFalse(DaitaFeature(state: state, settings: settings(daita: false)).isEnabled)
    }

    /// `isDaita` is the toggle, not a grant, so reading it cannot tell the chip
    /// anything the settings do not already say. Pinned so nobody wires the
    /// chip back onto it believing it means "running".
    func testTheChipDoesNotReadTheStateFlagThatOnlyMirrorsTheToggle() {
        let on = settings(daita: true)
        for isDaita in [true, false] {
            let feature = DaitaFeature(state: connected(isDaita: isDaita), settings: on)
            XCTAssertTrue(
                feature.isEnabled,
                "the chip changed with isDaita, which carries no grant")
        }
    }
}
