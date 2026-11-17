//
//  AccessMethodsTests.swift
//  MullvadVPN
//
//  Created by Jon Petersson on 2024-09-30.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import XCTest

class AccessMethodsTests: LoggedOutUITestCase {
    func testDirect() throws {
        HeaderBar(app)
            .tapSettingsButton()

        SettingsPage(app)
            .tapAPIAccessCell()

        APIAccessPage(app)
            .getAccessMethodCell(accessibilityId: AccessibilityIdentifier.accessMethodDirectCell)
            .tap()

        EditAccessMethodPage(app)
            .tapTestMethodButton()
            .verifyTestStatus(.reachable)
    }

    func testCanNotDisableLast() throws {
        HeaderBar(app)
            .tapSettingsButton()

        SettingsPage(app)
            .tapAPIAccessCell()

        APIAccessPage(app)
            .getAccessMethodCell(
                accessibilityId: AccessibilityIdentifier.accessMethodDirectCell
            )
            .tap()

        EditAccessMethodPage(app)
            .verifySwitchDisabled()
    }
}
