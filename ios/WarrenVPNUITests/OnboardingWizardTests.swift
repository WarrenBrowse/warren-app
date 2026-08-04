//
//  OnboardingWizardTests.swift
//  WarrenVPNUITests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  First-launch onboarding wizard (desktop parity): the wizard runs
//  AFTER the wallet login, as Welcome -> Wallet backup reminder ->
//  Subscription -> Preferences -> Done, pushed on a navigation stack
//  with the native back chevron, plus a "Skip wizard" escape on every
//  step but the last. Fully client-side up to the subscription step,
//  so no partner API is needed.
//

import Foundation
import XCTest

/// Drives the app to the wallet login chooser WITHOUT pre-seeding
/// onboarding-complete, so finishing the wallet creation lands in the
/// onboarding wizard (the fresh-install path).
final class OnboardingWizardTests: WalletFlowUITestCase {
    /// No pre-seed: the wizard must appear after the wallet is created.
    override class var appPreferencesSeed: Set<UITestAppPreferencesKey> {
        []
    }

    /// Optional visual QA: when `WARREN_QA_SHOT_DIR` is set (pass as
    /// `TEST_RUNNER_WARREN_QA_SHOT_DIR` to xcodebuild), write a PNG of
    /// the current screen there. No-op in normal test runs.
    private func snap(_ name: String) {
        guard let dir = ProcessInfo.processInfo.environment["WARREN_QA_SHOT_DIR"] else { return }
        let png = XCUIScreen.main.screenshot().pngRepresentation
        try? png.write(to: URL(fileURLWithPath: dir).appendingPathComponent("\(name).png"))
    }

    private func createWalletAndEnterWizard() {
        XCTAssertTrue(
            landsOnWalletLoginChooser(),
            "Expected the wallet login chooser on a fresh install (TOS agreed by the base setUp)"
        )
        snap("login-00-chooser")
        WalletLoginPage(app)
            .tapCreateWalletButton()
        let displayPage = WalletMnemonicDisplayPage(app)
        snap("login-01-backup-gate")
        displayPage.tapConfirmWrittenDownButton()

        XCTAssertTrue(
            app.buttons[AccessibilityIdentifier.onboardingWelcomeNextButton]
                .existsAfterWait(timeout: .extremelyLong),
            "The onboarding wizard welcome step should follow wallet creation on a fresh install"
        )
    }

    /// The back chevron works on every pushed step: subscription ->
    /// wallet reminder -> welcome, then forward again.
    func testWizardStepsNavigateBackAndForward() throws {
        createWalletAndEnterWizard()

        snap("onboarding-01-welcome")
        app.buttons[AccessibilityIdentifier.onboardingWelcomeNextButton].tap()

        // Wallet backup reminder: acknowledge, continue.
        let ackToggle = app.buttons[AccessibilityIdentifier.onboardingWalletAcknowledgeToggle]
        XCTAssertTrue(ackToggle.existsAfterWait(timeout: .long), "Backup reminder step missing")
        snap("onboarding-02-wallet-reminder")
        let continueButton = app.buttons[AccessibilityIdentifier.onboardingWalletContinueButton]
        XCTAssertFalse(
            continueButton.isEnabled,
            "Continue must be gated behind the backup acknowledgement"
        )
        ackToggle.tap()
        XCTAssertTrue(continueButton.isEnabled, "Continue should unlock after acknowledging")
        continueButton.tap()

        // Subscription step reached; go back twice with the nav chevron.
        let verifyButton = app.buttons[AccessibilityIdentifier.onboardingSubscriptionLaterCheck]
        XCTAssertTrue(verifyButton.existsAfterWait(timeout: .long), "Subscription step missing")
        snap("onboarding-03-subscription")

        let backButton = app.navigationBars.buttons.element(boundBy: 0)
        XCTAssertTrue(backButton.existsAfterWait(timeout: .short), "Back chevron missing on subscription step")
        backButton.tap()
        XCTAssertTrue(
            app.buttons[AccessibilityIdentifier.onboardingWalletContinueButton].existsAfterWait(timeout: .long),
            "Back from subscription should return to the wallet reminder"
        )
        app.navigationBars.buttons.element(boundBy: 0).tap()
        XCTAssertTrue(
            app.buttons[AccessibilityIdentifier.onboardingWelcomeNextButton].existsAfterWait(timeout: .long),
            "Back from the wallet reminder should return to welcome"
        )

        // Forward again: the acknowledgement is remembered by the model.
        app.buttons[AccessibilityIdentifier.onboardingWelcomeNextButton].tap()
        XCTAssertTrue(
            app.buttons[AccessibilityIdentifier.onboardingWalletContinueButton].existsAfterWait(timeout: .long),
            "Forward navigation should still reach the wallet reminder"
        )
    }

    /// "Skip wizard (advanced)" completes the onboarding and reaches the
    /// main UI without a subscription.
    func testSkipWizardReachesMain() throws {
        createWalletAndEnterWizard()

        app.buttons[AccessibilityIdentifier.onboardingSkipButton].tap()

        XCTAssertTrue(
            app.buttons[AccessibilityIdentifier.selectLocationButton]
                .existsAfterWait(timeout: .extremelyLong),
            "Skipping the wizard should land on the main (tunnel) screen"
        )
    }

    /// Full happy path through every wizard step, using a FUNDED wallet
    /// restored from the `WARREN_QA_MNEMONIC` environment variable (pass
    /// as `TEST_RUNNER_WARREN_QA_MNEMONIC`; never committed). The
    /// subscription check must succeed to unlock preferences + done.
    /// Skipped when the variable is absent, so CI never needs a secret.
    func testFullWizardWalkWithFundedWallet() throws {
        guard let mnemonic = ProcessInfo.processInfo.environment["WARREN_QA_MNEMONIC"],
              !mnemonic.isEmpty
        else {
            throw XCTSkip("WARREN_QA_MNEMONIC not set; full wizard walk needs a funded wallet")
        }

        XCTAssertTrue(landsOnWalletLoginChooser(), "Expected the wallet login chooser")
        WalletLoginPage(app)
            .tapRestoreWalletButton()
        WalletMnemonicInputPage(app)
            .enterFullPhrase(mnemonic)
            .tapRestoreWalletButton()

        XCTAssertTrue(
            app.buttons[AccessibilityIdentifier.onboardingWelcomeNextButton]
                .existsAfterWait(timeout: .extremelyLong),
            "The wizard should follow the wallet restore on a fresh install"
        )
        app.buttons[AccessibilityIdentifier.onboardingWelcomeNextButton].tap()

        let ackToggle = app.buttons[AccessibilityIdentifier.onboardingWalletAcknowledgeToggle]
        XCTAssertTrue(ackToggle.existsAfterWait(timeout: .long), "Backup reminder step missing")
        ackToggle.tap()
        app.buttons[AccessibilityIdentifier.onboardingWalletContinueButton].tap()

        // The funded wallet passes the subscription check and advances.
        let verifyButton = app.buttons[AccessibilityIdentifier.onboardingSubscriptionLaterCheck]
        XCTAssertTrue(verifyButton.existsAfterWait(timeout: .long), "Subscription step missing")
        verifyButton.tap()

        let prefsContinue = app.buttons[AccessibilityIdentifier.onboardingPreferencesContinueButton]
        XCTAssertTrue(
            prefsContinue.existsAfterWait(timeout: .extremelyLong),
            "An active subscription should advance to the preferences step"
        )
        snap("onboarding-04-preferences")
        prefsContinue.tap()

        let finishButton = app.buttons[AccessibilityIdentifier.onboardingDoneFinishButton]
        XCTAssertTrue(finishButton.existsAfterWait(timeout: .long), "Done step missing")
        snap("onboarding-05-done")
        finishButton.tap()

        XCTAssertTrue(
            app.buttons[AccessibilityIdentifier.selectLocationButton]
                .existsAfterWait(timeout: .extremelyLong),
            "Finishing the wizard should land on the main (tunnel) screen"
        )
        snap("onboarding-06-main")
    }
}

/// Regression guard for the DAITA relay-capability flag: the `/v1/exits`
/// projection (and the prebundled relay list) must advertise
/// DAITA-capable relays, because the relay selector filters on that flag
/// when the DAITA setting is on. With `daita: false` everywhere, every
/// DAITA-enabled install failed to connect with
/// `noRelaysSatisfyingConstraints` (2026-07-17). The simulator runs the
/// REAL relay selector against the bundled list, so this is
/// deterministic and needs no backend.
final class DAITARelaySelectionTests: WalletFlowUITestCase {
    func testConnectWithDAITAEnabledSelectsARelay() throws {
        XCTAssertTrue(landsOnWalletLoginChooser(), "Expected the wallet login chooser")
        WalletLoginPage(app)
            .tapCreateWalletButton()
        WalletMnemonicDisplayPage(app)
            .tapConfirmWrittenDownButton()

        // The in-app notification prompt covers the main screen right
        // after login; existence queries see through it but taps do not.
        if app.otherElements[.notificationPromptView].existsAfterWait(timeout: .default) {
            NotificationPromptPage(app).tapSkipButton()
        }

        // Onboarding-complete is pre-seeded by the base class, so the
        // wallet creation lands directly on the main screen.
        XCTAssertTrue(
            app.buttons[AccessibilityIdentifier.selectLocationButton]
                .existsAfterWait(timeout: .extremelyLong),
            "Expected the main (tunnel) screen after wallet creation"
        )

        HeaderBar(app).tapSettingsButton()
        SettingsPage(app).tapDAITACell()
        DAITAPage(app)
            .tapEnableSwitch()
            .tapEnableDialogButtonIfPresent()
        // DAITAPage.tapBackButton matches the back button by the English
        // "Settings" title and silently no-ops on localized simulators;
        // the nav-bar chevron works in every locale.
        app.navigationBars.buttons.element(boundBy: 0).tap()
        SettingsPage(app).tapDoneButton()

        TunnelControlPage(app)
            .tapConnectButton()
            .waitForConnectedLabel()
    }
}

/// The out-of-time gate: with no active subscription (a fresh wallet is
/// never funded) and the QA subscription override disabled, skipping the
/// wizard must land on the out-of-time screen, not the connect UI
/// (desktop parity with the expired-account redirect).
final class OutOfTimeGateTests: WalletFlowUITestCase {
    override class var appPreferencesSeed: Set<UITestAppPreferencesKey> {
        []
    }

    override class var assumeSubscribed: Bool { false }

    func testUnfundedWalletLandsOnOutOfTimeAfterWizardSkip() throws {
        XCTAssertTrue(
            landsOnWalletLoginChooser(),
            "Expected the wallet login chooser on a fresh install (TOS agreed by the base setUp)"
        )
        WalletLoginPage(app)
            .tapCreateWalletButton()
        WalletMnemonicDisplayPage(app).tapConfirmWrittenDownButton()

        XCTAssertTrue(
            app.buttons[AccessibilityIdentifier.onboardingWelcomeNextButton]
                .existsAfterWait(timeout: .extremelyLong),
            "The onboarding wizard welcome step should follow wallet creation"
        )
        app.buttons[AccessibilityIdentifier.onboardingSkipButton].tap()

        XCTAssertTrue(
            app.otherElements[AccessibilityIdentifier.outOfTimeView]
                .existsAfterWait(timeout: .extremelyLong),
            "An unfunded wallet must be gated on the out-of-time screen, not the connect UI"
        )
    }
}
