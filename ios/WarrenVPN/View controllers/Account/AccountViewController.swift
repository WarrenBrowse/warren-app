//
//  AccountViewController.swift
//  MullvadVPN
//
//  Created by pronebird on 20/03/2019.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import WarrenLogging
import WarrenSettings
import UIKit

enum AccountViewControllerAction: Sendable {
    case finish
    case logOut
    case navigateToVoucher
    case navigateToDeleteAccount
    case showPurchaseOptions
}

class AccountViewController: UIViewController, @unchecked Sendable {
    typealias ActionHandler = (AccountViewControllerAction) -> Void

    private let interactor: AccountInteractor

    private let contentView: AccountContentView = {
        let contentView = AccountContentView()
        return contentView
    }()

    private var paymentState: PaymentState = .none

    var actionHandler: ActionHandler?

    init(interactor: AccountInteractor) {
        self.interactor = interactor

        super.init(nibName: nil, bundle: nil)
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    // MARK: - View lifecycle

    override var preferredStatusBarStyle: UIStatusBarStyle {
        .lightContent
    }

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .secondaryColor

        navigationItem.title = NSLocalizedString("Account", comment: "")

        navigationItem.rightBarButtonItem = UIBarButtonItem(
            barButtonSystemItem: .done,
            target: self,
            action: #selector(handleDismiss)
        )

        contentView.accountTokenRowView.copyAccountNumber = { [weak self] in
            self?.copyAccountToken()
        }

        interactor.didReceiveTunnelState = { [weak self] in
            guard let self else { return }
            Task { @MainActor in
                applyViewState(animated: true)
            }
        }

        interactor.didReceiveDeviceState = { [weak self] deviceState in
            Task { @MainActor in
                self?.updateView(from: deviceState)
            }
        }

        configUI()
        addActions()
        updateView(from: interactor.deviceState)
        applyViewState(animated: false)
    }

    // MARK: - Private

    private func configUI() {
        view.addConstrainedSubviews([contentView]) {
            contentView.pinEdgesToSuperview()
        }
    }

    private func addActions() {
        contentView.purchaseButton.addTarget(self, action: #selector(requestStoreProducts), for: .touchUpInside)
        contentView.logoutButton.addTarget(self, action: #selector(logOut), for: .touchUpInside)
        contentView.deleteButton.addTarget(self, action: #selector(deleteAccount), for: .touchUpInside)
        contentView.debugOptionsButton.addTarget(self, action: #selector(showDebugOptions), for: .touchUpInside)
    }

    private func updateView(from deviceState: DeviceState) {
        guard case let .loggedIn(accountData, _) = deviceState else {
            return
        }

        // Show the Warren wallet SS58 identity in place of the Mullvad
        // account number; fall back to the stored number if unavailable.
        contentView.accountTokenRowView.accountNumber = interactor.walletAddress ?? accountData.number
        contentView.accountExpiryRowView.value = accountData.expiry
    }

    private func applyViewState(animated: Bool) {
        let isInteractionEnabled = paymentState.allowsViewInteraction

        contentView.purchaseButton.isEnabled =
            isInteractionEnabled
            && !interactor.tunnelState.isBlockingInternet
        contentView.accountTokenRowView.setButtons(enabled: isInteractionEnabled)
        contentView.logoutButton.isEnabled = isInteractionEnabled
        contentView.deleteButton.isEnabled = isInteractionEnabled
        contentView.debugOptionsButton.isEnabled = isInteractionEnabled
        navigationItem.rightBarButtonItem?.isEnabled = isInteractionEnabled

        view.isUserInteractionEnabled = isInteractionEnabled
        isModalInPresentation = !isInteractionEnabled

        navigationItem.setHidesBackButton(!isInteractionEnabled, animated: animated)
    }

    private func copyAccountToken() {
        guard let address = interactor.walletAddress ?? interactor.deviceState.accountData?.number else {
            return
        }

        UIPasteboard.general.string = address
    }

    // MARK: - Actions

    @objc private func logOut() {
        actionHandler?(.logOut)
    }

    @objc private func handleDismiss() {
        actionHandler?(.finish)
    }

    @objc private func redeemVoucher() {
        actionHandler?(.navigateToVoucher)
    }

    @objc private func deleteAccount() {
        actionHandler?(.navigateToDeleteAccount)
    }

    @objc private func requestStoreProducts() {
        actionHandler?(.showPurchaseOptions)
    }

    @objc func showDebugOptions() {
        let localizedString = NSLocalizedString("Debug options", comment: "")

        let sheetController = UIAlertController(
            title: localizedString,
            message: nil,
            preferredStyle: UIDevice.current.userInterfaceIdiom == .pad ? .alert : .actionSheet
        )
        sheetController.overrideUserInterfaceStyle = .dark
        sheetController.view.tintColor = .AlertController.tintColor

        sheetController.addAction(
            UIAlertAction(
                title: "Redeem voucher",
                style: .default,
                handler: { _ in
                    self.redeemVoucher()
                }
            )
        )

        #if DEBUG
            let gotaTunEnabled = PacketTunnelDebugSettings.useGotaTun
            sheetController.addAction(
                UIAlertAction(
                    title: "Use GotaTun: \(gotaTunEnabled ? "ON" : "OFF")",
                    style: .default,
                    handler: { [weak self] _ in
                        PacketTunnelDebugSettings.useGotaTun = !gotaTunEnabled
                        self?.interactor.tunnelManager.reapplyTunnelConfiguration()
                    }
                )
            )
        #endif

        sheetController.addAction(
            UIAlertAction(
                title: "Cancel",
                style: .cancel
            )
        )

        present(sheetController, animated: true)
    }
}
