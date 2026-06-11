//
//  AccountCoordinator.swift
//  MullvadVPN
//
//  Created by Jon Petersson on 2023-04-14.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import WarrenREST
import Routing
import SafariServices
import UIKit

enum AccountDismissReason: Equatable, Sendable {
    case none
    case userLoggedOut
    case accountDeletion
}

final class AccountCoordinator: Coordinator, Presentable, Presenting, @unchecked Sendable {
    private let interactor: AccountInteractor
    private let storePaymentManager: StorePaymentManager
    private var accountController: AccountViewController?

    let navigationController: UINavigationController
    var presentedViewController: UIViewController {
        navigationController
    }

    var didFinish: (@MainActor (AccountCoordinator, AccountDismissReason) -> Void)?

    init(
        navigationController: UINavigationController,
        interactor: AccountInteractor,
        storePaymentManager: StorePaymentManager
    ) {
        self.navigationController = navigationController
        self.interactor = interactor
        self.storePaymentManager = storePaymentManager
    }

    func start(animated: Bool) {
        navigationController.navigationBar.prefersLargeTitles = true

        let accountController = AccountViewController(
            interactor: interactor
        )

        accountController.actionHandler = handleViewControllerAction

        navigationController.pushViewController(accountController, animated: animated)
        self.accountController = accountController
    }

    private func handleViewControllerAction(_ action: AccountViewControllerAction) {
        switch action {
        case .finish:
            didFinish?(self, .none)
        case .logOut:
            logOut()
        case .navigateToVoucher:
            navigateToRedeemVoucher()
        case .navigateToDeleteAccount:
            navigateToDeleteAccount()
        case .showPurchaseOptions:
            didRequestShowInAppPurchase(paymentAction: .purchase)
        }
    }

    /// Presents the native Apple StoreKit purchase flow. The credit goes
    /// through warren-api signed with the wallet key; there is no Mullvad
    /// account number to pass (the backend identifies the wallet from the
    /// request signature).
    private func didRequestShowInAppPurchase(paymentAction: PaymentAction) {
        let coordinator = InAppPurchaseCoordinator(
            storePaymentManager: storePaymentManager,
            paymentAction: paymentAction
        )
        coordinator.didFinish = { coordinator in
            coordinator.dismiss(animated: true)
        }
        coordinator.start()
        presentChild(coordinator, animated: true)
    }

    /// Opens the Warren Stripe checkout funnel as a secondary "buy on the
    /// web" path. The user buys a plan on `checkout.warrenbrowse.com`,
    /// receives a voucher, and redeems it in-app (Redeem voucher). The
    /// funnel is stateless, so no wallet identifier is passed in the URL.
    private func openCheckout() {
        guard let url = URL(string: "https://checkout.warrenbrowse.com/") else { return }
        let safari = SFSafariViewController(url: url)
        safari.preferredBarTintColor = .Warren.navy
        safari.preferredControlTintColor = .Warren.yellow
        navigationController.present(safari, animated: true)
    }

    private func navigateToRedeemVoucher() {
        let coordinator = ProfileVoucherCoordinator(
            navigationController: CustomNavigationController(),
            interactor: RedeemVoucherInteractor(
                tunnelManager: interactor.tunnelManager
            )
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

    @MainActor
    private func navigateToDeleteAccount() {
        let coordinator = AccountDeletionCoordinator(
            navigationController: CustomNavigationController(),
            tunnelManager: interactor.tunnelManager
        )

        coordinator.start()
        coordinator.didConclude = { accountDeletionCoordinator, success in
            Task { @MainActor in
                accountDeletionCoordinator.dismiss(
                    animated: true,
                    completion: {
                        if success { self.didFinish?(self, .userLoggedOut) }
                    }
                )
            }
        }

        presentChild(
            coordinator,
            animated: true,
            configuration: ModalPresentationConfiguration(
                preferredContentSize: UIMetrics.AccountDeletion.preferredContentSize,
                modalPresentationStyle: .custom,
                transitioningDelegate: FormSheetTransitioningDelegate(
                    options: FormSheetPresentationOptions(
                        useFullScreenPresentationInCompactWidth: true,
                        adjustViewWhenKeyboardAppears: false
                    ))
            )
        )
    }

    // MARK: - Alerts

    private func logOut() {
        let presentation = AlertPresentation(
            id: "account-logout-alert",
            accessibilityIdentifier: .logOutSpinnerAlertView,
            icon: .spinner,
            message: nil,
            buttons: []
        )

        let alertPresenter = AlertPresenter(context: self)

        Task {
            await interactor.logout()
            DispatchQueue.main.asyncAfter(deadline: .now() + .seconds(1)) { [weak self] in
                guard let self else { return }

                alertPresenter.dismissAlert(presentation: presentation, animated: true)
                self.didFinish?(self, .userLoggedOut)
            }
        }

        alertPresenter.showAlert(presentation: presentation, animated: true)
    }
}
