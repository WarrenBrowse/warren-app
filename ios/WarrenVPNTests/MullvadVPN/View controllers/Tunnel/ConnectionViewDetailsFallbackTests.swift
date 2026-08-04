//
//  ConnectionViewDetailsFallbackTests.swift
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

final class ConnectionViewDetailsFallbackTests: XCTestCase {
    // Non-localizable placeholder names so NSLocalizedString passes them through
    // verbatim regardless of the test simulator's locale.
    private func relay(hostname: String, country: String, city: String) -> SelectedRelay {
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
                countryCode: "zz",
                city: city,
                cityCode: "zz1",
                latitude: 0,
                longitude: 0
            ),
            isIPOverridden: false,
            features: nil
        )
    }

    private func makeViewModel(state: TunnelState) -> ConnectionViewViewModel {
        ConnectionViewViewModel(
            tunnelStatus: TunnelStatus(state: state),
            relayConstraints: RelayConstraints(),
            relayCache: MockRelayCache(),
            customListRepository: CustomListRepository()
        )
    }

    func testOutFallsBackToExitLocationWhenConnected() {
        let exit = relay(hostname: "zz-exit-1", country: "Testland", city: "Testcity")
        let viewModel = makeViewModel(
            state: .connected(
                SelectedRelays(entry: nil, exit: exit, retryAttempt: 0),
                isPostQuantum: false,
                isDaita: false
            )
        )

        // No egress conncheck: the Out value is the exit location, not an IP.
        XCTAssertEqual(viewModel.outExitLocation, "Testcity, Testland")
        XCTAssertEqual(viewModel.tunnelProtocolName, "QUIC")
        XCTAssertFalse(viewModel.isMultihop)
    }

    func testMultihopFlagWhenEntryRelayPresent() {
        let entry = relay(hostname: "zz-entry-1", country: "Entryland", city: "Entrycity")
        let exit = relay(hostname: "zz-exit-1", country: "Testland", city: "Testcity")
        let viewModel = makeViewModel(
            state: .connected(
                SelectedRelays(entry: entry, exit: exit, retryAttempt: 0),
                isPostQuantum: false,
                isDaita: false
            )
        )

        XCTAssertTrue(viewModel.isMultihop)
        // Out still reflects the EXIT location, not the entry.
        XCTAssertEqual(viewModel.outExitLocation, "Testcity, Testland")
    }

    func testNoProtocolOrLocationWhenDisconnected() {
        let viewModel = makeViewModel(state: .disconnected)

        XCTAssertNil(viewModel.tunnelProtocolName)
        XCTAssertNil(viewModel.outExitLocation)
        XCTAssertFalse(viewModel.isMultihop)
    }
}
