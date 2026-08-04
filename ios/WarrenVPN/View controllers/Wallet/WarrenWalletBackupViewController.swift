//
//  WarrenWalletBackupViewController.swift
//  WarrenVPN
//
//  Created by Warren on 2026-05-21.
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  UIKit shell hosting `WarrenMnemonicDisplayView` (SwiftUI) for the
//  Settings → "View recovery phrase" flow. Gated by Face ID / Touch ID
//  / passcode (`LAContext.deviceOwnerAuthentication`) before the
//  Keychain entry is read.
//

import SwiftUI
import UIKit

protocol WarrenWalletBackupViewControllerDelegate: AnyObject {
    /// Called when the user dismisses the backup screen.
    func walletBackupControllerDidFinish(_ controller: WarrenWalletBackupViewController)
}

final class WarrenWalletBackupViewController: UIViewController {
    weak var delegate: WarrenWalletBackupViewControllerDelegate?

    private let interactor: WarrenWalletInteractor
    private var hostingController: UIHostingController<AnyView>?

    private let activityIndicator: UIActivityIndicatorView = {
        let indicator = UIActivityIndicatorView(style: .large)
        indicator.color = .Warren.yellow
        indicator.hidesWhenStopped = true
        indicator.translatesAutoresizingMaskIntoConstraints = false
        return indicator
    }()

    init(interactor: WarrenWalletInteractor) {
        self.interactor = interactor
        super.init(nibName: nil, bundle: nil)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) is not supported")
    }

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .Warren.navy
        title = NSLocalizedString(
            "Recovery phrase",
            tableName: "Wallet",
            comment: "Title shown above the wallet backup view"
        )

        navigationItem.rightBarButtonItem = UIBarButtonItem(
            barButtonSystemItem: .done,
            target: self,
            action: #selector(handleDone)
        )

        view.addSubview(activityIndicator)
        NSLayoutConstraint.activate([
            activityIndicator.centerXAnchor.constraint(equalTo: view.centerXAnchor),
            activityIndicator.centerYAnchor.constraint(equalTo: view.centerYAnchor),
        ])
    }

    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        authenticateAndReveal()
    }

    private func authenticateAndReveal() {
        let reason = NSLocalizedString(
            "Reveal the recovery phrase for your Warren wallet",
            tableName: "Wallet",
            comment: "Reason shown in the Face ID / Touch ID system prompt"
        )
        activityIndicator.startAnimating()
        interactor.loadMnemonicWithAuth(reason: reason) { [weak self] result in
            guard let self else { return }
            self.activityIndicator.stopAnimating()
            switch result {
            case .success(let phrase):
                self.installDisplayView(mnemonic: phrase)
            case .failure(let error):
                self.presentErrorAlert(error)
            }
        }
    }

    private func installDisplayView(mnemonic: String) {
        let displayView = WarrenMnemonicDisplayView(
            mnemonic: mnemonic,
            onConfirmed: { [weak self] in
                guard let self else { return }
                self.delegate?.walletBackupControllerDidFinish(self)
            }
        )
        let hosting = UIHostingController(rootView: AnyView(displayView))
        hosting.view.backgroundColor = .Warren.navy
        addChild(hosting)
        hosting.view.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(hosting.view)
        NSLayoutConstraint.activate([
            hosting.view.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor),
            hosting.view.bottomAnchor.constraint(equalTo: view.safeAreaLayoutGuide.bottomAnchor),
            hosting.view.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            hosting.view.trailingAnchor.constraint(equalTo: view.trailingAnchor),
        ])
        hosting.didMove(toParent: self)
        hostingController = hosting
    }

    private func presentErrorAlert(_ error: WarrenWalletInteractorError) {
        let message: String
        switch error {
        case .authenticationFailed(let m):
            message = m
        case .noWallet:
            message = NSLocalizedString(
                "No wallet is currently provisioned on this device. Generate or import one first.",
                tableName: "Wallet",
                comment: ""
            )
        case .keychain:
            message = NSLocalizedString(
                "Failed to read the wallet from secure storage.",
                tableName: "Wallet",
                comment: ""
            )
        default:
            message = NSLocalizedString(
                "An unexpected error occurred.",
                tableName: "Wallet",
                comment: ""
            )
        }
        let alert = UIAlertController(
            title: NSLocalizedString("Cannot reveal phrase", tableName: "Wallet", comment: ""),
            message: message,
            preferredStyle: .alert
        )
        alert.addAction(
            UIAlertAction(
                title: NSLocalizedString("Close", tableName: "Wallet", comment: ""),
                style: .default,
                handler: { [weak self] _ in
                    guard let self else { return }
                    self.delegate?.walletBackupControllerDidFinish(self)
                }
            )
        )
        present(alert, animated: true)
    }

    @objc private func handleDone() {
        if let delegate {
            delegate.walletBackupControllerDidFinish(self)
        } else if let nav = navigationController {
            // Fallback when embedded directly in a UINavigationController
            // without a Coordinator delegate (Settings flow injection).
            nav.popViewController(animated: true)
        } else {
            dismiss(animated: true)
        }
    }
}
