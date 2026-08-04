//
//  SettingsLoggedOutTests.swift
//  WarrenVPNUITests
//
//  Created by Niklas Berglund on 2024-02-23.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import Foundation
import XCTest

class SettingsLoggedInTests: LoggedInWithTimeUITestCase {
    func testLanguageSelection() throws {
        HeaderBar(app)
            .tapSettingsButton()

        TunnelControlPage(app)
            .tapConnectButton()
            .waitForConnectedLabel()

        SettingsPage(app)
            .tapLanguageCell()
            .dismissAlert()
            .tapDoneButton()

        TunnelControlPage(app)
            .tapDisconnectButton()
    }
}
