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
    let tunnelManager: TunnelManager
    private let storePaymentManager: StorePaymentManager

    nonisolated(unsafe) var didFinishPayment: (@Sendable (OutOfTimeCoordinator) -> Void)?

    var presentedViewController: UIViewController {
        navigationController
    }

    private(set) var isMakingPayment = false
    private var viewController: OutOfTimeViewController?

    init(
        navigationController: RootContainerViewController,
        tunnelManager: TunnelManager,
        storePaymentManager: StorePaymentManager
    ) {
        self.navigationController = navigationController
        self.tunnelManager = tunnelManager
        self.storePaymentManager = storePaymentManager
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
            interactor: interactor
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

    /// Primary path: the native Apple StoreKit purchase, credited to the
    /// wallet through warren-api (the backend identifies the wallet from
    /// the signed payment session, no account number is passed).
    func didRequestShowInAppPurchase() {
        let coordinator = InAppPurchaseCoordinator(
            storePaymentManager: storePaymentManager,
            paymentAction: .purchase
        )
        coordinator.didFinish = { coordinator in
            coordinator.dismiss(animated: true)
        }
        coordinator.start()
        presentChild(coordinator, animated: true)
    }

    /// Secondary path: the Stripe checkout funnel on the web. The user
    /// buys a plan on `checkout.warrenbrowse.com`, receives a voucher,
    /// and redeems it in-app. The funnel is stateless, so no wallet
    /// identifier is passed in the URL.
    func didRequestOpenWebCheckout() {
        guard let url = URL(string: "https://checkout.warrenbrowse.com/") else { return }
        let safari = SFSafariViewController(url: url)
        safari.preferredBarTintColor = .Warren.navy
        safari.preferredControlTintColor = .Warren.yellow
        navigationController.present(safari, animated: true)
    }

    /// Voucher path: the same in-app redemption flow reachable from the
    /// account screen (desktop parity: the expired view offers a Redeem
    /// voucher button). A successful redemption credits the wallet and the
    /// account-expiry refresh routes the user off the out-of-time gate.
    func didRequestRedeemVoucher() {
        let coordinator = ProfileVoucherCoordinator(
            navigationController: CustomNavigationController(),
            interactor: RedeemVoucherInteractor(tunnelManager: tunnelManager)
        )
        coordinator.didFinish = { coordinator in
            coordinator.dismiss(animated: true)
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
}
