//
//  AccountCoordinator.swift
//  MullvadVPN
//
//  Created by Jon Petersson on 2023-04-14.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import WarrenREST
import Routing
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
            confirmLogout()
        case .navigateToVoucher:
            navigateToRedeemVoucher()
        case .navigateToBackup:
            navigateToBackup()
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

    /// Pushes the Face ID gated recovery-phrase backup view. The
    /// controller pops itself when used without a delegate (same
    /// injection the Settings flow relies on).
    private func navigateToBackup() {
        navigationController.pushViewController(
            WarrenWalletBackupViewController(interactor: WarrenWalletInteractor()),
            animated: true
        )
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

    /// Logging out is a true sign-out: the recovery phrase is erased
    /// from this device, and without a backup the account and its
    /// remaining time are unrecoverable. Gate the action behind an
    /// explicit confirmation offering the backup view first (mirrors
    /// the desktop backed-up gate).
    private func confirmLogout() {
        let presentation = AlertPresentation(
            id: "account-logout-confirm-alert",
            icon: .alert,
            title: NSLocalizedString("Log out of this account?", comment: ""),
            message: [
                NSLocalizedString(
                    "Logging out erases this account from this device. There is no email "
                        + "or password to log back in, your recovery phrase is the ONLY way "
                        + "to restore it.",
                    comment: ""
                ),
                NSLocalizedString(
                    "If you have not backed up your recovery phrase, your subscription will "
                        + "be lost permanently.",
                    comment: ""
                ),
            ].joined(separator: "\n\n"),
            // Gate the destructive action behind an explicit acknowledgement,
            // mirroring the desktop backed-up checkbox.
            checkbox: AlertCheckbox(
                title: NSLocalizedString(
                    "I have backed up my recovery phrase and understand this account will "
                        + "be removed from this device.",
                    comment: ""
                ),
                accessibilityId: .logOutBackupConfirmationCheckbox
            ),
            buttons: [
                AlertAction(
                    title: NSLocalizedString("Back up my phrase", comment: ""),
                    style: .default,
                    handler: { [weak self] in
                        self?.navigateToBackup()
                    }
                ),
                AlertAction(
                    title: NSLocalizedString("Log out", comment: ""),
                    style: .destructive,
                    accessibilityId: .logOutDeviceConfirmButton,
                    isGatedByCheckbox: true,
                    handler: { [weak self] in
                        self?.logOut()
                    }
                ),
                AlertAction(
                    title: NSLocalizedString("Cancel", comment: ""),
                    style: .default,
                    accessibilityId: .logOutDeviceCancelButton
                ),
            ]
        )

        let alertPresenter = AlertPresenter(context: self)
        alertPresenter.showAlert(presentation: presentation, animated: true)
    }

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
