//
//  WelcomeCoordinator.swift
//  MullvadVPN
//
//  Created by Mojgan on 2023-06-28.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import Routing
import SafariServices
import UIKit

final class WelcomeCoordinator: Coordinator, Poppable, Presenting {
    private let navigationController: RootContainerViewController
    private let tunnelManager: TunnelManager

    private var viewController: WelcomeViewController?

    var didFinish: (() -> Void)?
    var didLogout: ((String) -> Void)?

    var presentedViewController: UIViewController {
        navigationController
    }

    init(
        navigationController: RootContainerViewController,
        tunnelManager: TunnelManager
    ) {
        self.navigationController = navigationController
        self.tunnelManager = tunnelManager
    }

    func start(animated: Bool) {
        let interactor = WelcomeInteractor(
            tunnelManager: tunnelManager
        )

        interactor.didAddMoreCredit = { [weak self] in
            self?.showSetupAccountCompleted()
        }

        let controller = WelcomeViewController(interactor: interactor)
        controller.delegate = self

        viewController = controller

        navigationController.pushViewController(controller, animated: animated)
    }

    func showSetupAccountCompleted() {
        let coordinator = SetupAccountCompletedCoordinator(navigationController: navigationController)
        coordinator.didFinish = { [weak self] coordinator in
            coordinator.removeFromParent()
            self?.didFinish?()
        }
        addChild(coordinator)
        coordinator.start(animated: true)
    }

    func popFromNavigationStack(animated: Bool, completion: (() -> Void)?) {
        guard let viewController,
            let index = navigationController.viewControllers.firstIndex(of: viewController)
        else {
            completion?()
            return
        }
        navigationController.setViewControllers(
            Array(navigationController.viewControllers[0..<index]),
            animated: animated,
            completion: completion
        )
    }
}

extension WelcomeCoordinator: @preconcurrency WelcomeViewControllerDelegate {
    func didRequestToShowFailToFetchProducts(controller: WelcomeViewController) {
        let message = NSLocalizedString("Failed to load products, please try again", comment: "")

        let presentation = AlertPresentation(
            id: "welcome-failed-to-fetch-products-alert",
            icon: .info,
            message: message,
            buttons: [
                AlertAction(
                    title: NSLocalizedString("Got it!", comment: ""),
                    style: .default
                )
            ]
        )

        let presenter = AlertPresenter(context: self)
        presenter.showAlert(presentation: presentation, animated: true)
    }

    func didRequestToShowInfo(controller: WelcomeViewController) {
        let message = [
            NSLocalizedString(
                "This is the name assigned to the device. Each device logged in on a "
                    + "Warren account gets a unique name that helps "
                    + "you identify it when you manage your devices in the app or on the website.",
                comment: ""
            ),
            NSLocalizedString(
                "You can have up to 3 devices connected at the same time on one Warren account.",
                comment: ""
            ),
            NSLocalizedString(
                "If you log out, the device and the device name is removed. "
                    + "When you log back in again, the device will get a new name.",
                comment: ""
            ),
        ].joinedParagraphs(lineBreaks: 1)

        let presentation = AlertPresentation(
            id: "welcome-device-name-alert",
            icon: .info,
            message: message,
            buttons: [
                AlertAction(
                    title: NSLocalizedString("Got it!", comment: ""),
                    style: .default
                )
            ]
        )

        let presenter = AlertPresenter(context: self)
        presenter.showAlert(presentation: presentation, animated: true)
    }

    func didRequestToViewPurchaseOptions(
        accountNumber: String
    ) {
        // Warren has no in-app purchase: the user buys a plan on the Stripe
        // checkout funnel and redeems the voucher in-app, aligned with the
        // account screen and desktop/Android.
        guard let url = URL(string: "https://checkout.warrenbrowse.com/") else { return }
        let safari = SFSafariViewController(url: url)
        safari.preferredBarTintColor = .Warren.navy
        safari.preferredControlTintColor = .Warren.yellow
        navigationController.present(safari, animated: true)
    }
}
