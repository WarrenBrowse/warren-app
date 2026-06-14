//
//  WalletFlowTests.swift
//  WarrenVPNUITests
//
//  Created by Warren on 2026-06-14.
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  Client-side wallet (BIP39) identity flow: create, restore, logout.
//  Unlike the deleted Mullvad account-number UI tests, this flow needs
//  NO partner API and NO temporary backend account: provisioning a
//  wallet is local (Rust BIP39 generation + iOS Keychain), so these
//  tests are deterministic.
//
//  Reachability precondition: the app routes to the wallet login chooser
//  (`WarrenWalletLoginCoordinator`) only when TOS is agreed AND the
//  Warren onboarding wizard has already completed AND no wallet exists
//  in the Keychain (see `nextWarrenRoutes` / `ApplicationCoordinator`).
//  `AppResetManager.resetKeychain()` wipes the wallet Keychain when the
//  `.settings` reset key is set, so a `.forceLoggedOut` + reset-all launch
//  guarantees the no-wallet half. The onboarding-complete half is pre-seeded
//  via `appPreferencesSeed` (a reset-only policy cannot SET a key), so the
//  route evaluator picks `.login` (the wallet chooser). TOS is auto-agreed by
//  the base `setUp`.
//

import Foundation
import XCTest

/// Base case that drives the app to the wallet login chooser: force
/// logged-out, reset everything (which also wipes the wallet Keychain), and
/// pre-seed onboarding-complete so the route evaluator picks `.login`.
class WalletFlowUITestCase: BaseUITestCase {
    override class var authenticationState: LaunchArguments.AuthenticationState {
        .forceLoggedOut
    }

    /// Reset the settings store (which also wipes the wallet Keychain via
    /// `AppResetManager`) so no wallet survives between runs.
    override class var settingsResetPolicy: UITestSettingsResetPolicy {
        .all
    }

    /// Reset every app preference, then pre-seed onboarding-complete below.
    override class var appPreferencesPolicy: UITestAppPreferencesPolicy {
        .all
    }

    /// Pre-seed onboarding-complete so the app routes to the wallet login
    /// chooser instead of replaying the onboarding wizard (a reset-only
    /// policy cannot SET this flag, only clear it).
    override class var appPreferencesSeed: Set<UITestAppPreferencesKey> {
        [.hasCompletedWarrenOnboarding]
    }

    /// A known-valid 12-word BIP39 test vector (the canonical all-"abandon"
    /// vector with the correct checksum word "about"). Used by the restore
    /// test so no real seed is ever committed.
    static let knownTestMnemonic =
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"

    /// Returns true if the wallet login chooser is currently shown. With the
    /// onboarding-complete pre-seed in place this is the expected landing
    /// screen, so callers assert on it rather than skip. Detected by the
    /// create-wallet button (the chooser's stable, queryable anchor).
    func landsOnWalletLoginChooser() -> Bool {
        app.buttons[AccessibilityIdentifier.walletCreateButton].existsAfterWait(timeout: .extremelyLong)
    }
}

final class WalletFlowTests: WalletFlowUITestCase {
    /// Create wallet: chooser -> generate -> 12 words shown -> confirm
    /// "I have written them down" -> wallet provisioned (leaves the
    /// chooser).
    func testCreateWalletShowsMnemonicAndConfirms() throws {
        XCTAssertTrue(
            landsOnWalletLoginChooser(),
            "Expected the wallet login chooser (onboarding-complete is pre-seeded and no wallet exists)"
        )

        WalletLoginPage(app)
            .tapCreateWalletButton()

        // The generated phrase is displayed directly with a backup gate.
        WalletMnemonicDisplayPage(app)
            .tapConfirmWrittenDownButton()

        // After confirming, the wallet is persisted and the login chooser
        // is dismissed. Assert we left the chooser screen.
        XCTAssertFalse(
            app.otherElements[.walletLoginView].existsAfterWait(timeout: .short),
            "Login chooser should be dismissed once the wallet is provisioned"
        )
    }

    /// Restore wallet: chooser -> "I already have a recovery phrase" ->
    /// paste a known 12-word phrase -> Restore -> wallet provisioned.
    func testRestoreWalletWithKnownPhrase() throws {
        XCTAssertTrue(
            landsOnWalletLoginChooser(),
            "Expected the wallet login chooser (onboarding-complete is pre-seeded and no wallet exists)"
        )

        WalletLoginPage(app)
            .tapRestoreWalletButton()

        WalletMnemonicInputPage(app)
            .enterFullPhrase(Self.knownTestMnemonic)
            .tapRestoreWalletButton()

        XCTAssertFalse(
            app.otherElements[.walletLoginView].existsAfterWait(timeout: .short),
            "Login chooser should be dismissed once the wallet is restored"
        )
    }

    /// The chooser exposes both entry points.
    func testLoginChooserExposesCreateAndRestore() throws {
        XCTAssertTrue(
            landsOnWalletLoginChooser(),
            "Expected the wallet login chooser (onboarding-complete is pre-seeded and no wallet exists)"
        )

        XCTAssertTrue(
            app.buttons[AccessibilityIdentifier.walletCreateButton].existsAfterWait(),
            "Create-wallet button missing from the login chooser"
        )
        XCTAssertTrue(
            app.buttons[AccessibilityIdentifier.walletRestoreButton].existsAfterWait(),
            "Restore-wallet button missing from the login chooser"
        )
    }
}
