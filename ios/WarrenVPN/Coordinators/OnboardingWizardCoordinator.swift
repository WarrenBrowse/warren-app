//
//  OnboardingWizardCoordinator.swift
//  WarrenVPN
//
//  Created by Warren on 2026-05-21.
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  Coordinator that owns the SwiftUI 5-step wizard
//  (`OnboardingWizardView`) and bridges its callbacks to the existing
//  `WarrenWalletCoordinator` (for generate / import) and to
//  `SFSafariViewController` (for the warrenbrowse.com subscription
//  link). Follows the Mullvad iOS Coordinator pattern from
//  `Routing/Coordinator.swift`.
//

import Routing
import SafariServices
import SwiftUI
import UIKit
import WarrenLogging

protocol OnboardingWizardCoordinatorDelegate: AnyObject {
    @MainActor func onboardingWizardDidFinish(_ coordinator: OnboardingWizardCoordinator)
}

final class OnboardingWizardCoordinator: Coordinator, Presentable {
    private let logger = Logger(label: "OnboardingWizardCoordinator")
    let navigationController: UINavigationController
    private let interactor: WarrenWalletInteractor
    private let state: OnboardingWizardState

    weak var delegate: OnboardingWizardCoordinatorDelegate?

    var presentedViewController: UIViewController {
        navigationController
    }

    init(
        navigationController: UINavigationController = UINavigationController(),
        interactor: WarrenWalletInteractor = WarrenWalletInteractor()
    ) {
        self.navigationController = navigationController
        self.interactor = interactor
        self.state = OnboardingWizardState()
    }

    func start(animated: Bool) {
        // Seed wallet-already-exists state so users who relaunched mid-
        // onboarding skip the wallet step on the path of least friction.
        state.hasWallet = interactor.walletExists()

        let view = OnboardingWizardView(
            state: state,
            onGenerateWallet: { [weak self] in self?.presentWalletGenerate() },
            onImportWallet: { [weak self] in self?.presentWalletImport() },
            onOpenSubscription: { [weak self] in self?.presentSubscriptionLink() },
            onFinish: { [weak self] in self?.finish() }
        )
        let host = UIHostingController(rootView: view)
        host.view.backgroundColor = .Warren.navy
        host.modalPresentationStyle = .fullScreen
        host.isModalInPresentation = true
        navigationController.setViewControllers([host], animated: false)
        navigationController.setNavigationBarHidden(true, animated: false)
    }

    // MARK: - Step routes

    private func presentWalletGenerate() {
        let coordinator = WarrenWalletCoordinator(
            navigationController: navigationController,
            interactor: interactor,
            entryPoint: .generate
        )
        coordinator.didFinish = { [weak self] coord, success in
            guard let self else { return }
            coord.removeFromParent()
            if success {
                self.state.hasWallet = true
                self.state.advance()
            }
            // Restore the hidden nav bar that the wallet coordinator un-
            // hides when pushing.
            self.navigationController.setNavigationBarHidden(true, animated: false)
        }
        addChild(coordinator)
        navigationController.setNavigationBarHidden(false, animated: false)
        coordinator.start(animated: true)
    }

    private func presentWalletImport() {
        let coordinator = WarrenWalletCoordinator(
            navigationController: navigationController,
            interactor: interactor,
            entryPoint: .importExisting
        )
        coordinator.didFinish = { [weak self] coord, success in
            guard let self else { return }
            coord.removeFromParent()
            if success {
                self.state.hasWallet = true
                self.state.advance()
            }
            self.navigationController.setNavigationBarHidden(true, animated: false)
        }
        addChild(coordinator)
        navigationController.setNavigationBarHidden(false, animated: false)
        coordinator.start(animated: true)
    }

    private func presentSubscriptionLink() {
        guard let url = URL(string: "https://checkout.warrenbrowse.com/") else {
            logger.error("Invalid subscription URL")
            return
        }
        let safari = SFSafariViewController(url: url)
        safari.preferredBarTintColor = .Warren.navy
        safari.preferredControlTintColor = .Warren.yellow
        navigationController.present(safari, animated: true)
    }

    private func finish() {
        delegate?.onboardingWizardDidFinish(self)
    }
}
