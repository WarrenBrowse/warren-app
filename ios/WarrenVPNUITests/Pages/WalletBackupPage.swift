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

        self.pageElement = app.textFields["walletMnemonicWordField_0"]
        waitForPageToBeShown()
    }

    /// Accessibility identifier of the word field at `index` (0-based).
    /// Mirrors the per-cell identifier set in `WarrenMnemonicInputView`.
    private func wordFieldIdentifier(_ index: Int) -> String {
        "walletMnemonicWordField_\(index)"
    }

    /// Pastes the full space-separated phrase into the first word field.
    /// `WarrenMnemonicInputView` splits on spaces and fills all 12 cells.
    @discardableResult func enterFullPhrase(_ mnemonic: String) -> Self {
        let firstField = app.textFields[wordFieldIdentifier(0)]
        firstField.tap()
        firstField.typeText(mnemonic)
        return self
    }

    /// Types each word into its own field (when paste-fill is not desired).
    @discardableResult func enterWords(_ words: [String]) -> Self {
        for (index, word) in words.enumerated() {
            let field = app.textFields[wordFieldIdentifier(index)]
            field.tap()
            field.typeText(word)
        }
        return self
    }

    @discardableResult func tapRestoreWalletButton() -> Self {
        app.buttons[AccessibilityIdentifier.walletMnemonicRestoreSubmitButton].tap()
        return self
    }
}
