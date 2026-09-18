//
//  DaitaTruthfulnessTests.swift
//  WarrenVPNTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import Network
import WarrenMockData
import WarrenREST
import WarrenRustRuntime
import WarrenSettings
import WarrenTypes
import XCTest

@testable import WarrenVPN

/// The connect screen showed a plain "DAITA" chip on every connection made with
/// the DAITA toggle on, and `TunnelState.isDaita` is that toggle plumbed
/// through, not a grant from the exit. So the chip asserted a protection that
/// was not on the wire.
///
/// The desktop daemon reads the exit's own echo before claiming anything
/// (`talpid-warren-tunnel/src/lib.rs`: "the Connected state never claims a
/// protection that is not running"), and the chip now reads the same echo:
/// `warren_tunnel_daita_active()` reports what the live session was granted.
/// The iOS datapath still dials with the defense off, so today that is always
/// false and the chip says so; the day it dials with DAITA on, the chip follows
/// the grant with no second change.
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

    func testTheChipClaimsDaitaOnlyWhenTheSessionWasGrantedIt() {
        let ungranted = DaitaFeature(
            state: connected(isDaita: true),
            settings: settings(daita: true),
            isGranted: false
        )
        XCTAssertEqual(ungranted.name, "DAITA: not active on this server")

        let granted = DaitaFeature(
            state: connected(isDaita: true),
            settings: settings(daita: true),
            isGranted: true
        )
        XCTAssertEqual(granted.name, "DAITA")
    }

    /// The datapath is the source, not the toggle: what the chip reports by
    /// default is what the live session carries, and with no session that is
    /// nothing.
    func testWithNoSessionTheDatapathReportsNoGrant() {
        XCTAssertFalse(WarrenQuinnAdapter.daitaActive())
        XCTAssertFalse(
            DaitaFeature(state: connected(isDaita: true), settings: settings(daita: true))
                .isGranted,
            "the chip defaulted to a grant nothing measured")
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
            let feature = DaitaFeature(
                state: connected(isDaita: isDaita), settings: on, isGranted: false)
            XCTAssertTrue(
                feature.isEnabled,
                "the chip changed with isDaita, which carries no grant")
            XCTAssertEqual(
                feature.name, "DAITA: not active on this server",
                "the wording changed with isDaita, which carries no grant")
        }
    }
}
