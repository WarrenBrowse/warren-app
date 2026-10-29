//
//  DeviceManagementPage.swift
//  WarrenVPNUITests
//
//  Created by Niklas Berglund on 2024-03-27.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import XCTest

/// Page class for the account's "Devices" list (warren-core doc 115), opened from Account and from
/// the device-limit and no-session-token banners.
class DeviceManagementPage: Page {
    override init(_ app: XCUIApplication) {
        super.init(app)

        self.pageElement =
            app
            .descendants(matching: .any)
            .matching(
                identifier: AccessibilityIdentifier.deviceManagementView.asString
            ).element
        waitForPageToBeShown()
    }

    @discardableResult func waitForNoLoading() -> Self {
        XCTAssertTrue(
            app.otherElements[.deviceRemovalProgressView]
                .notExistsAfterWait(timeout: .longerThanMullvadAPITimeout)
        )

        return self
    }

    @discardableResult func waitForDeviceList() -> Self {
        XCTAssertTrue(
            app
                .descendants(matching: .any)[AccessibilityIdentifier.deviceManagementView]
                .existsAfterWait(timeout: .long)
        )

        return self
    }

    /// Taps "Disconnect" on the `index`th other device, this device's row having none.
    @discardableResult func tapRemoveDeviceButton(index: Int) -> Self {
        app.buttons
            .matching(identifier: AccessibilityIdentifier.deviceCellRemoveButton.asString)
            .element(boundBy: index)
            .tap()

        return self
    }

    @discardableResult public func verifyCurrentDeviceExists() -> Self {
        XCTAssertTrue(
            app.staticTexts["This device"]
                .existsAfterWait()
        )

        return self
    }

    /// This device's row offers "Log out" rather than "Disconnect".
    @discardableResult public func verifyCurrentDeviceCannotBeRemoved() -> Self {
        let rows = app.descendants(matching: .any)
            .matching(identifier: AccessibilityIdentifier.deviceCell.asString)
        let removeButtons = app.buttons
            .matching(identifier: AccessibilityIdentifier.deviceCellRemoveButton.asString)

        XCTAssertEqual(removeButtons.count, rows.count - 1)
        XCTAssertTrue(app.buttons[AccessibilityIdentifier.logoutButton].exists)

        return self
    }

    @discardableResult public func verifyRemovableDeviceCount(_ expectedCount: Int) -> Self {
        XCTAssertEqual(
            app.buttons.matching(
                identifier: AccessibilityIdentifier.deviceCellRemoveButton.asString
            )
            .count,

            expectedCount
        )
        return self
    }
}

/// Confirmation alert displayed before disconnecting another device
class DeviceManagementLogOutDeviceConfirmationAlert: Page {
    override init(_ app: XCUIApplication) {
        super.init(app)
        self.pageElement = app.otherElements[.alertContainerView]
        waitForPageToBeShown()
    }

    @discardableResult func tapDisconnectButton() -> Self {
        app.buttons[AccessibilityIdentifier.logOutDeviceConfirmButton]
            .tap()
        return self
    }
}
