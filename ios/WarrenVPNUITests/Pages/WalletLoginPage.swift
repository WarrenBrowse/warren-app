//
//  WalletLoginPage.swift
//  WarrenVPNUITests
//
//  Created by Warren on 2026-06-14.
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  Page object for the wallet (BIP39) login chooser shown by
//  `WarrenWalletLoginCoordinator`: "Generate new wallet" /
//  "I already have a recovery phrase". This screen is entirely
//  client-side (no partner API), so it is faithfully testable.
//

import Foundation
import XCTest

class WalletLoginPage: Page {
    @discardableResult override init(_ app: XCUIApplication) {
        super.init(app)

        self.pageElement = app.buttons[.walletCreateButton]
        waitForPageToBeShown()
    }

    @discardableResult func tapCreateWalletButton() -> Self {
        app.buttons[AccessibilityIdentifier.walletCreateButton].tap()
        return self
    }

    @discardableResult func tapRestoreWalletButton() -> Self {
        app.buttons[AccessibilityIdentifier.walletRestoreButton].tap()
        return self
    }
}
