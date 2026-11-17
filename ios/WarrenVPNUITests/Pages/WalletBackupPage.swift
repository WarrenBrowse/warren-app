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
        // acknowledgement row, then asks for three words of the phrase; both
        // are answered here so callers do not have to care about the gate.
        let words = displayedWords()
        let toggle = app.buttons[AccessibilityIdentifier.walletBackupAcknowledgeToggle]
        let gated = toggle.exists
        if gated, toggle.value as? String == "0" {
            toggle.tap()
        }
        app.buttons[AccessibilityIdentifier.walletMnemonicConfirmButton].tap()
        if gated {
            answerBackupCheck(words: words)
        }
        return self
    }

    /// The phrase as the grid shows it, by one-based position: each word cell
    /// reads "<position>. <word>" to VoiceOver.
    private func displayedWords() -> [Int: String] {
        var words: [Int: String] = [:]
        let cells = app.staticTexts.matching(NSPredicate(format: "label MATCHES %@", "^[0-9]+\\. [a-z]+$"))
        for cell in cells.allElementsBoundByIndex {
            let parts = cell.label.split(separator: " ")
            if let position = Int(parts[0].dropLast()) {
                words[position] = String(parts[1])
            }
        }
        return words
    }

    /// Types back the words the check asks for, each field labelled with its
    /// one-based position, and continues.
    private func answerBackupCheck(words: [Int: String]) {
        let fields = app.textFields.matching(identifier: AccessibilityIdentifier.walletBackupCheckWordField.asString)
        XCTAssertTrue(fields.firstMatch.waitForExistence(timeout: 5))
        for field in fields.allElementsBoundByIndex {
            let digits = field.label.filter(\.isNumber)
            guard let position = Int(digits), let word = words[position] else {
                XCTFail("backup check field without a known position: \(field.label)")
                return
            }
            field.tap()
            field.typeText(word)
        }
        app.buttons[AccessibilityIdentifier.walletBackupCheckContinueButton].tap()
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
