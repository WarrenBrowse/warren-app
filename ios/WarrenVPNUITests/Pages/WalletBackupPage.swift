//
//  WalletBackupPage.swift
//  WarrenVPNUITests
//
//  Created by Warren on 2026-06-14.
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  Page objects for the two BIP39 wallet screens:
//    - `WalletMnemonicDisplayPage` (generated phrase + "I have written
//      them down" confirm), shown after "Generate new wallet".
//    - `WalletMnemonicInputPage` (12-word grid + "Restore wallet"),
//      shown after "I already have a recovery phrase".
//  Both are fully client-side (Keychain only), so no partner API is
//  needed to drive them.
//

import Foundation
import XCTest

/// The freshly generated 12-word recovery phrase display + backup gate.
class WalletMnemonicDisplayPage: Page {
    @discardableResult override init(_ app: XCUIApplication) {
        super.init(app)

        self.pageElement = app.buttons[.walletMnemonicConfirmButton]
        waitForPageToBeShown()
    }

    @discardableResult func tapCopyButton() -> Self {
        app.buttons[AccessibilityIdentifier.walletMnemonicCopyButton].tap()
        return self
    }

    /// Ticks the "I have written down my recovery phrase" acknowledgement
    /// that gates the confirm button on the creation backup screen.
    @discardableResult func tapAcknowledgeToggle() -> Self {
        app.buttons[AccessibilityIdentifier.walletBackupAcknowledgeToggle].tap()
        return self
    }

    @discardableResult func tapConfirmWrittenDownButton() -> Self {
        // The creation flow gates the confirm button behind the explicit
        // acknowledgement row; tick it first when it is present so
        // callers do not have to care about the gate.
        let toggle = app.buttons[AccessibilityIdentifier.walletBackupAcknowledgeToggle]
        if toggle.exists, toggle.value as? String == "0" {
            toggle.tap()
        }
        app.buttons[AccessibilityIdentifier.walletMnemonicConfirmButton].tap()
        return self
    }
}

/// The 12-word recovery phrase input grid + restore submit.
class WalletMnemonicInputPage: Page {
    @discardableResult override init(_ app: XCUIApplication) {
        super.init(app)

        self.pageElement = app.textViews[AccessibilityIdentifier.walletMnemonicPhraseField]
        waitForPageToBeShown()
    }

    /// Types the whole space-separated phrase into the one field the screen
    /// has. There used to be twelve, which is why a 24-word phrase could not
    /// be entered at all.
    @discardableResult func enterFullPhrase(_ mnemonic: String) -> Self {
        let field = app.textViews[AccessibilityIdentifier.walletMnemonicPhraseField]
        field.tap()
        field.typeText(mnemonic)
        return self
    }

    /// Whether the Restore button is offered for what is currently typed.
    var isRestoreEnabled: Bool {
        app.buttons[AccessibilityIdentifier.walletMnemonicRestoreSubmitButton].isEnabled
    }

    @discardableResult func tapRestoreWalletButton() -> Self {
        app.buttons[AccessibilityIdentifier.walletMnemonicRestoreSubmitButton].tap()
        return self
    }
}
