//
//  OutOfTimeCoordinator.swift
//  MullvadVPN
//
//  Created by pronebird on 10/03/2023.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import Routing
import SafariServices
import UIKit

class OutOfTimeCoordinator: Coordinator, Presenting, @preconcurrency OutOfTimeViewControllerDelegate, Poppable {
    let navigationController: RootContainerViewController
    let storePaymentManager: StorePaymentManager
    let tunnelManager: TunnelManager

    nonisolated(unsafe) var didFinishPayment: (@Sendable (OutOfTimeCoordinator) -> Void)?

    var presentedViewController: UIViewController {
        navigationController
    }

    private(set) var isMakingPayment = false
    private var viewController: OutOfTimeViewController?

    init(
        navigationController: RootContainerViewController,
        storePaymentManager: StorePaymentManager,
        tunnelManager: TunnelManager
    ) {
        self.navigationController = navigationController
        self.storePaymentManager = storePaymentManager
        self.tunnelManager = tunnelManager
    }

    func start(animated: Bool) {
        let interactor = OutOfTimeInteractor(
            tunnelManager: tunnelManager
        )

        interactor.didAddMoreCredit = { [weak self] in
            guard let self else { return }
            didFinishPayment?(self)
        }

        let controller = OutOfTimeViewController(
            interactor: interactor,
            errorPresenter: PaymentAlertPresenter(alertContext: self)
        )

        controller.delegate = self

        viewController = controller

        navigationController.pushViewController(controller, animated: animated)
    }

    func popFromNavigationStack(animated: Bool, completion: (() -> Void)?) {
        guard let viewController else {
            completion?()
            return
        }

        let viewControllers = navigationController.viewControllers.filter { $0 != viewController }

        navigationController.setViewControllers(
            viewControllers,
            animated: animated,
            completion: completion
        )
    }

    // MARK: - OutOfTimeViewControllerDelegate

    func didRequestShowInAppPurchase(
        accountNumber: String,
        paymentAction: PaymentAction
    ) {
        // Warren has no in-app purchase: the user buys a plan on the Stripe
        // checkout funnel and redeems the voucher in-app, aligned with the
        // account screen and desktop/Android. The funnel is stateless, so no
        // wallet identifier is passed in the URL.
        guard let url = URL(string: "https://checkout.warrenbrowse.com/") else { return }
        let safari = SFSafariViewController(url: url)
        safari.preferredBarTintColor = .Warren.navy
        safari.preferredControlTintColor = .Warren.yellow
        navigationController.present(safari, animated: true)
    }
}
