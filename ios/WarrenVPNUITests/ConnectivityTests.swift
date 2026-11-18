//
//  ConnectivityTests.swift
//  WarrenVPNUITests
//
//  Created by Niklas Berglund on 2024-01-18.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import Foundation
import Network
import XCTest

class ConnectivityTests: LoggedOutUITestCase {
    let firewallAPIClient = FirewallClient()

    /// Verifies that the app still functions when API has been blocked
    func testAPIConnectionViaBridges() throws {
        firewallAPIClient.removeRules()
        let hasTimeAccountNumber = getAccountWithTime()

        addTeardownBlock {
            self.deleteTemporaryAccountWithTime(accountNumber: hasTimeAccountNumber)
            self.firewallAPIClient.removeRules()
        }

        try Networking.verifyCanAccessAPI()  // Just to make sure there's no old firewall rule still active
        firewallAPIClient.createRule(try FirewallRule.makeBlockAPIAccessFirewallRule())
        try Networking.verifyCannotAccessAPI()

        login(accountNumber: hasTimeAccountNumber)

        HeaderBar(app)
            .verifyDeviceLabelShown()
    }

    /// Verifies that after connecting to a relay, the app can still reach the API to log out
    func testAPIReachableWhenBlocked() throws {
        let hasTimeAccountNumber = getAccountWithTime()
        addTeardownBlock {
            self.deleteTemporaryAccountWithTime(accountNumber: hasTimeAccountNumber)
        }

        login(accountNumber: hasTimeAccountNumber)

        TunnelControlPage(app)
            .tapSelectLocationButton()

        // Select the first country, its first city and its first relay
        SelectLocationPage(app)
            .tapLocationCellExpandButton(withName: BaseUITestCase.testsDefaultCountryName)
            // Must be a little specific here in order to avoid using relay services country with experimental relays
            .tapLocationCellExpandButton(withName: BaseUITestCase.testsDefaultMullvadOwnedCityName)
            .tapLocationCell(withName: BaseUITestCase.testsDefaultMullvadOwnedRelayName)

        allowAddVPNConfigurationsIfAsked()

        TunnelControlPage(app)
            .tapSelectLocationButton()

        SelectLocationPage(app)
            .tapDoneButton()

        // Log out as a way of verifying that the API can be reached
        HeaderBar(app)
            .tapAccountButton()

        AccountPage(app)
            .tapLogOutButton()

        LoginPage(app)
    }

    /// Test that the app is functioning when API is down. To simulate API being down we create a dummy access method
    func testAppStillFunctioningWhenAPIDown() throws {
        let hasTimeAccountNumber = getAccountWithTime()
        let customAccessMethodName = "Disable-access-dummy"

        addTeardownBlock {
            self.deleteTemporaryAccountWithTime(accountNumber: hasTimeAccountNumber)
        }

        login(accountNumber: hasTimeAccountNumber)

        TunnelControlPage(app)

        HeaderBar(app)
            .tapSettingsButton()

        SettingsPage(app)
            .tapAPIAccessCell()

        APIAccessPage(app)
            .tapAddButton()

        allowLocalNetworkAccessIfAsked()

        AddAccessMethodPage(app)
            .tapNameCell()
            .enterText(customAccessMethodName)
            .tapTypeCell()
            .tapSOCKS5TypeValueCell()
            .tapServerCell()
            .enterText("123.123.123.123")
            .dismissKeyboard()
            .tapPortCell()
            .enterText("123")
            .dismissKeyboard()
            .tapAddButton()
            .waitForAPIUnreachableLabel()

        AddAccessMethodAPIUnreachableAlert(app)
            .tapSaveButton()

        disableBuiltinAccessMethods()

        SettingsPage(app)
            .swipeDownToDismissModal()

        // Actual test. Make sure it is possible to connect to a relay
        TunnelControlPage(app)
            .tapConnectButton()

        allowAddVPNConfigurationsIfAsked()

        TunnelControlPage(app)
            .waitForConnectedLabel()

        HeaderBar(app)
            .tapAccountButton()

        // Log out will take long because API cannot be reached
        AccountPage(app)
            .tapLogOutButton()

        // Verify API cannot be reached by doing a login attempt which should fail
        LoginPage(app)
            .tapAccountNumberTextField()
            .enterText(hasTimeAccountNumber)
            .tapAccountNumberSubmitButton()
            .verifyFailIconShown()
    }

    func testIfLocalNetworkSharingIsBlocking() throws {
        let skipReason = """
            This test is currently skipped since there is no way to allow local network access for UI tests.
            Since its blocked by the system, there is no way of testing the `Local network sharing` switch.
            Non of these solutions worked: https://developer.apple.com/forums/thread/668729
            """
        try XCTSkipIf(true, skipReason)
        let hasTimeAccountNumber = getAccountWithTime()
        addTeardownBlock {
            self.deleteTemporaryAccountWithTime(accountNumber: hasTimeAccountNumber)
        }

        login(accountNumber: hasTimeAccountNumber)

        TunnelControlPage(app)
            .tapConnectButton()

        allowAddVPNConfigurationsIfAsked()

        TunnelControlPage(app)
            .waitForConnectedLabel()

        try Networking.verifyCannotAccessLocalNetwork()

        HeaderBar(app)
            .tapSettingsButton()

        SettingsPage(app)
            .tapVPNSettingsCell()
            .tapIncludeAllNetworksCell()

        IncludeAllNetworksPage(app)
            .tapEnableLocalNetworkSharing()
            .tapBackButton()

        SettingsPage(app)
            .tapDoneButton()

        TunnelControlPage(app)
            .waitForConnectedLabel()

        try Networking.verifyCanAccessLocalNetwork()
    }

    /// Toggle enabled switch for all existing access methods.
    /// Preconditions:
    /// - The app is currently showing API access view.
    /// - There is one custom access method enabled
    /// - The extra access method is not disabled
    private func disableBuiltinAccessMethods() {
        var accessMethods = APIAccessPage(app).getAccessMethodCells()
        accessMethods.removeLast()
        for cell in accessMethods {
            cell.tap()
            EditAccessMethodPage(app)
                .tapEnableMethodSwitch()
                .tapBackButton()
        }
    }

    /// Toggle enabled switch for all existing access methods if the switch is in off state. It is a precondition that the app is currently showing API access view.
    private func toggleAllAccessMethodsEnabledSwitchesIfOff() {
        for cell in APIAccessPage(app).getAccessMethodCells() {
            cell.tap()
            EditAccessMethodPage(app)
                .tapEnableMethodSwitchIfOff()
                .tapBackButton()
        }
    }
}
