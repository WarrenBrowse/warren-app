//
//  OnboardingWizardCoordinator.swift
//  WarrenVPN
//
//  Created by Warren on 2026-05-21.
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  Coordinator for the first-launch onboarding wizard, mirroring the
//  desktop flow (`views/onboarding/*`): the wallet already exists (the
//  login screen created or restored it), so the steps are Welcome ->
//  Wallet backup reminder -> Subscription -> Privacy preferences ->
//  Done, pushed on this coordinator's own UINavigationController for
//  native back navigation. Skipping (the "Skip wizard" link) marks the
//  onboarding complete, same as the desktop skip.
//

import Routing
import SafariServices
import SwiftUI
import UIKit
import WarrenLogging
import WarrenSettings

protocol OnboardingWizardCoordinatorDelegate: AnyObject {
    @MainActor func onboardingWizardDidFinish(_ coordinator: OnboardingWizardCoordinator)
}

final class OnboardingWizardCoordinator: Coordinator, Presentable, Presenting {
    private let logger = Logger(label: "OnboardingWizardCoordinator")
    let navigationController: UINavigationController
    private let tunnelManager: TunnelManager
    private let interactor: WarrenWalletInteractor
    private let storePaymentManager: StorePaymentManager
    private let model = OnboardingWizardModel()

    weak var delegate: OnboardingWizardCoordinatorDelegate?

    var presentedViewController: UIViewController {
        navigationController
    }

    init(
        tunnelManager: TunnelManager,
        storePaymentManager: StorePaymentManager,
        navigationController: UINavigationController = UINavigationController(),
        interactor: WarrenWalletInteractor = WarrenWalletInteractor()
    ) {
        self.tunnelManager = tunnelManager
        self.storePaymentManager = storePaymentManager
        self.navigationController = navigationController
        self.interactor = interactor
    }

    func start(animated: Bool) {
        configureNavigationBar()
        seedPreferences()

        let welcome = OnboardingWelcomeStepView(
            onContinue: { [weak self] in self?.pushWallet() },
            onSkip: { [weak self] in self?.finish() }
        )
        navigationController.setViewControllers([host(welcome)], animated: false)
    }

    // MARK: - Steps

    private func pushWallet() {
        model.mnemonicError = nil
        if model.mnemonic == nil {
            interactor.loadMnemonicForOnboardingBackup { [weak self] result in
                guard let self else { return }
                switch result {
                case .success(let phrase):
                    self.model.mnemonic = phrase
                case .failure(let error):
                    self.logger.error("Onboarding backup reminder failed to load the phrase: \(error)")
                    self.model.mnemonicError = String(
                        localized: "Could not read your recovery phrase. Restart the app and try again.",
                        table: "Onboarding"
                    )
                }
            }
        }

        let step = OnboardingWalletStepView(
            model: model,
            onContinue: { [weak self] in self?.pushSubscription() },
            onSkip: { [weak self] in self?.finish() }
        )
        navigationController.pushViewController(host(step), animated: true)
    }

    private func pushSubscription() {
        model.subscriptionError = nil
        let step = OnboardingSubscriptionStepView(
            model: model,
            onPurchase: { [weak self] in self?.presentInAppPurchase() },
            onOpenWeb: { [weak self] in self?.presentSubscriptionLink() },
            onRedeemVoucher: { [weak self] in self?.presentRedeemVoucher() },
            onVerify: { [weak self] in self?.verifySubscription(quiet: false) },
            onSkip: { [weak self] in self?.finish() }
        )
        navigationController.pushViewController(host(step), animated: true)
    }

    private func pushPreferences() {
        let step = OnboardingPreferencesStepView(
            model: model,
            onContinue: { [weak self] in self?.applyPreferencesAndContinue() },
            onSkip: { [weak self] in self?.finish() }
        )
        navigationController.pushViewController(host(step), animated: true)
    }

    private func pushDone() {
        let step = OnboardingDoneStepView(
            onFinish: { [weak self] in self?.finish() }
        )
        navigationController.pushViewController(host(step), animated: true)
    }

    // MARK: - Subscription

    /// Primary payment path: the native Apple StoreKit purchase. When the
    /// purchase sheet closes, quietly re-check the subscription so a
    /// successful purchase advances the wizard without another tap.
    private func presentInAppPurchase() {
        let coordinator = InAppPurchaseCoordinator(
            storePaymentManager: storePaymentManager,
            paymentAction: .purchase
        )
        coordinator.didFinish = { [weak self] coordinator in
            coordinator.dismiss(animated: true)
            self?.verifySubscription(quiet: true)
        }
        coordinator.start()
        presentChild(coordinator, animated: true)
    }

    /// Secondary payment path: the Stripe checkout funnel on the web
    /// (buy a plan, receive a voucher, redeem it in-app).
    private func presentSubscriptionLink() {
        let safari = SFSafariViewController(url: URL(string: "https://checkout.warrenbrowse.com/")!)
        safari.preferredBarTintColor = .Warren.navy
        safari.preferredControlTintColor = .Warren.yellow
        navigationController.present(safari, animated: true)
    }

    /// Voucher path: buy on the web, receive a voucher, redeem it in-app.
    /// Reuses the same redemption flow as the account screen. On success the
    /// wallet is credited, so re-check the subscription to advance the wizard
    /// (quiet: a cancelled voucher entry should not raise the inline error).
    private func presentRedeemVoucher() {
        let coordinator = ProfileVoucherCoordinator(
            navigationController: CustomNavigationController(),
            interactor: RedeemVoucherInteractor(tunnelManager: tunnelManager)
        )
        coordinator.didFinish = { [weak self] coordinator in
            coordinator.dismiss(animated: true)
            self?.verifySubscription(quiet: true)
        }
        coordinator.didCancel = { coordinator in
            coordinator.dismiss(animated: true)
        }
        coordinator.start()
        presentChild(
            coordinator,
            animated: true,
            configuration: ModalPresentationConfiguration(
                preferredContentSize: UIMetrics.SettingsRedeemVoucher.preferredContentSize,
                modalPresentationStyle: .custom,
                transitioningDelegate: FormSheetTransitioningDelegate(
                    options: FormSheetPresentationOptions(
                        useFullScreenPresentationInCompactWidth: false,
                        adjustViewWhenKeyboardAppears: true
                    ))
            )
        )
    }

    /// Checks the wallet's subscription against warren-api and advances
    /// to the preferences step when it is active. `quiet` suppresses the
    /// inline error for automatic checks (after the purchase sheet
    /// closes) where "no subscription" usually just means the user
    /// cancelled the sheet.
    private func verifySubscription(quiet: Bool) {
        guard !model.subscriptionChecking else { return }
        model.subscriptionChecking = true
        model.subscriptionError = nil
        interactor.fetchSubscriptionExpiry { [weak self] result in
            Task { @MainActor in
                guard let self else { return }
                self.model.subscriptionChecking = false
                switch result {
                case .success(let expiry) where expiry > Date():
                    // Refresh the account chrome (expiry label, footer)
                    // before entering the main UI.
                    self.tunnelManager.updateAccountData()
                    self.pushPreferences()
                case .success:
                    if !quiet {
                        self.model.subscriptionError = String(
                            localized: "No active subscription found. Please purchase one first.",
                            table: "Onboarding"
                        )
                    }
                case .failure(let error):
                    self.logger.error("Subscription check failed: \(error)")
                    if !quiet {
                        self.model.subscriptionError = String(
                            localized: "Could not check subscription status. Please try again.",
                            table: "Onboarding"
                        )
                    }
                }
            }
        }
    }

    // MARK: - Preferences

    private func seedPreferences() {
        let settings = tunnelManager.settings
        model.multiHopAlways = settings.tunnelMultihopState == .always
        model.daitaEnabled = settings.daita.isEnabled
    }

    /// Persists the chosen defenses through the same settings pipeline
    /// as the Settings screens. Multi-hop OFF maps back to `.whenNeeded`
    /// (the adaptive default), never `.never`.
    private func applyPreferencesAndContinue() {
        var updates: [TunnelSettingsUpdate] = []
        let settings = tunnelManager.settings

        let multihop: MultihopState = model.multiHopAlways ? .always : .whenNeeded
        if settings.tunnelMultihopState != multihop {
            updates.append(.multihop(multihop))
        }

        var daita = settings.daita
        if daita.isEnabled != model.daitaEnabled {
            daita.isEnabled = model.daitaEnabled
            updates.append(.daita(daita))
        }

        if !updates.isEmpty {
            tunnelManager.updateSettings(updates)
        }
        pushDone()
    }

    // MARK: - Helpers

    private func host(_ view: some View) -> UIViewController {
        let host = UIHostingController(rootView: view)
        host.view.backgroundColor = .Warren.navy
        // The step content carries the large title (desktop parity); the
        // bar itself stays chrome-only for the back chevron.
        host.navigationItem.title = ""
        return host
    }

    private func configureNavigationBar() {
        let appearance = UINavigationBarAppearance()
        appearance.configureWithTransparentBackground()
        appearance.backgroundColor = .Warren.navy
        navigationController.navigationBar.standardAppearance = appearance
        navigationController.navigationBar.scrollEdgeAppearance = appearance
        navigationController.navigationBar.tintColor = .white
        navigationController.setNavigationBarHidden(false, animated: false)
    }

    private func finish() {
        delegate?.onboardingWizardDidFinish(self)
    }
}
