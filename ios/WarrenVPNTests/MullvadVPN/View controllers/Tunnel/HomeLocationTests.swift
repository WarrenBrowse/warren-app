//
//  HomeLocationTests.swift
//  WarrenVPNTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import WarrenTypes
import XCTest

@testable import WarrenVPN

/// The location row names what desktop's `location-name.ts` names, and pictures the country with
/// the round flag `LocationFlag.tsx` draws.
final class HomeLocationTests: XCTestCase {
    private let connected = TunnelState.connected(HomeStatusTests.helsinki(), isPostQuantum: false, isDaita: false)

    func testATunnelNamesTheExitCountryFirst() {
        let location = HomeLocation(tunnelState: connected, exitLocations: .any, selectedName: nil)
        XCTAssertEqual(location.name, "Finland, Helsinki")
    }

    func testACityStateExitIsNamedOnce() {
        let state = TunnelState.connecting(HomeStatusTests.singapore, isPostQuantum: false, isDaita: false)
        XCTAssertEqual(HomeLocation(tunnelState: state, exitLocations: .any, selectedName: nil).name, "Singapore")
    }

    func testWithNoTunnelTheRowNamesThePick() {
        let picked = RelayConstraint.only(UserSelectedRelays(locations: [.country("fr")]))
        XCTAssertEqual(
            HomeLocation(tunnelState: .disconnected, exitLocations: .any, selectedName: nil).name, "Automatic")
        XCTAssertEqual(
            HomeLocation(tunnelState: .disconnected, exitLocations: picked, selectedName: "France").name, "France")
        XCTAssertEqual(
            HomeLocation(tunnelState: .disconnected, exitLocations: picked, selectedName: nil).name, "Select location")
    }

    /// Once a tunnel is up or coming up, the flag is the exit's.
    func testATunnelShowsTheFlagOfTheExitCountry() {
        let location = HomeLocation(tunnelState: connected, exitLocations: .any, selectedName: nil)
        XCTAssertEqual(location.flagCode, "fi")
    }

    func testWithNoTunnelTheFlagIsThePickedCountrys() {
        let picked = RelayConstraint.only(UserSelectedRelays(locations: [.city("nl", "ams")]))
        let location = HomeLocation(tunnelState: .disconnected, exitLocations: picked, selectedName: "Amsterdam")
        XCTAssertEqual(location.flagCode, "nl")
    }

    /// Automatic and a custom list name no country: the row shows only its label.
    func testAPickThatNamesNoCountryHasNoFlag() {
        XCTAssertNil(HomeLocation(tunnelState: .disconnected, exitLocations: .any, selectedName: nil).flagCode)
        let list = UserSelectedRelays(
            locations: [.country("fi")],
            customListSelection: UserSelectedRelays.CustomListSelection(listId: UUID(), isList: true)
        )
        XCTAssertNil(
            HomeLocation(tunnelState: .disconnected, exitLocations: .only(list), selectedName: "Work").flagCode)
    }

    /// The round flags ship in the asset catalog under the relay list's ISO codes, whatever their
    /// case; a code without a flag draws nothing rather than a blank disc.
    func testTheFlagIsTheRoundFlagOfTheCatalog() {
        XCTAssertNotNil(HomeLocationFlagView.image(countryCode: "fi"))
        XCTAssertNotNil(HomeLocationFlagView.image(countryCode: "GB"))
        XCTAssertNil(HomeLocationFlagView.image(countryCode: "zz"))
    }

    func testAWholeCustomListNamesNoCountry() {
        let list = UserSelectedRelays(
            locations: [.country("fi")],
            customListSelection: UserSelectedRelays.CustomListSelection(listId: UUID(), isList: true)
        )
        XCTAssertNil(HomeLocation.countryCode(of: list))
        XCTAssertEqual(HomeLocation.countryCode(of: UserSelectedRelays(locations: [.hostname("de", "ber", "x")])), "de")
    }

    /// The hero shows the country the tunnel goes to: the exit once chosen, the pick while
    /// connecting before that, and the last exit while the tunnel waits for a network.
    func testTheHeroTargetsTheCountryTheTunnelGoesTo() {
        let picked = RelayConstraint.only(UserSelectedRelays(locations: [.country("fr")]))
        XCTAssertEqual(TunnelViewController.targetCountry(tunnelState: connected, exitLocations: picked), "Finland")
        XCTAssertEqual(
            TunnelViewController.targetCountry(
                tunnelState: .connecting(nil, isPostQuantum: false, isDaita: false), exitLocations: picked),
            "fr")
        XCTAssertNil(TunnelViewController.targetCountry(tunnelState: .disconnected, exitLocations: picked))
        XCTAssertEqual(
            TunnelViewController.targetCountry(
                tunnelState: .waitingForConnectivity(.noNetwork), exitLocations: .any, lastExitCountry: "Finland"),
            "Finland")
        XCTAssertNil(
            TunnelViewController.targetCountry(
                tunnelState: .waitingForConnectivity(.noConnection), exitLocations: .any, lastExitCountry: "Finland"))
    }
}
