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
            interactor: interactor,
            errorPresenter: PaymentAlertPresenter(alertContext: self)
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
        case .restorePurchasesInfo:
            showRestorePurchasesInfo()
        case .showFailedToLoadProducts:
            showFailToFetchProducts()
        case .showRestorePurchases:
            openCheckout()
        case .showPurchaseOptions:
            openCheckout()
        }
    }

    /// Opens the Warren Stripe checkout funnel. Aligned with desktop and
    /// Android: there is no in-app purchase. The user buys a plan on
    /// `checkout.warrenbrowse.com`, receives a voucher, and redeems it
    /// in-app (Redeem voucher). The funnel is stateless, so no wallet
    /// identifier is passed in the URL.
    private func openCheckout() {
        guard let url = URL(string: "https://checkout.warrenbrowse.com/") else { return }
        let safari = SFSafariViewController(url: url)
        safari.preferredBarTintColor = .Warren.navy
        safari.preferredControlTintColor = .Warren.yellow
        navigationController.present(safari, animated: true)
    }

    private func didRequestShowInAppPurchase(
        paymentAction: PaymentAction
    ) {
        guard let accountNumber = interactor.deviceState.accountData?.number else { return }
        let coordinator = InAppPurchaseCoordinator(
            storePaymentManager: storePaymentManager,
            accountNumber: accountNumber,
            paymentAction: paymentAction
        )
        coordinator.didFinish = { coordinator in
            coordinator.dismiss(animated: true)
        }
        coordinator.start()
        presentChild(coordinator, animated: true)
    }

    private func navigateToRedeemVoucher() {
        let coordinator = ProfileVoucherCoordinator(
            navigationController: CustomNavigationController(),
            interactor: RedeemVoucherInteractor(
                tunnelManager: interactor.tunnelManager,
                accountsProxy: interactor.accountsProxy,
                verifyVoucherAsAccount: false
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

    private func showRestorePurchasesInfo() {
        let message = NSLocalizedString(
            """
            You can use the "restore purchases" function to check for any in-app payments \
            made via Apple services. If there is a payment that has not been credited, it will \
            add the time to the currently logged in Warren account.
            """,
            comment: ""
        )

        let presentation = AlertPresentation(
            id: "account-device-info-alert",
            icon: .info,
            title: NSLocalizedString("If you haven’t received additional VPN time after purchasing", comment: ""),
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

    func showFailToFetchProducts() {
        let message = NSLocalizedString(
            "Failed to load products, please try again",
            comment: ""
        )

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
}
