//
//  WarrenWalletImportViewController.swift
//  WarrenVPN
//
//  Created by Warren on 2026-05-21.
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  UIKit shell hosting `WarrenMnemonicInputView` (SwiftUI). Used both
//  in onboarding (Step 2b: import existing wallet) and from the Login
//  screen ("Restore from recovery phrase").
//

import SwiftUI
import UIKit

protocol WarrenWalletImportViewControllerDelegate: AnyObject {
    /// Called after the user enters a valid 12-word phrase and the
    /// interactor has successfully persisted it.
    func walletImportController(_ controller: WarrenWalletImportViewController, didImportMnemonic mnemonic: String)

    /// Called if the user backs out of the import flow.
    func walletImportControllerDidCancel(_ controller: WarrenWalletImportViewController)
}

final class WarrenWalletImportViewController: UIViewController {
    weak var delegate: WarrenWalletImportViewControllerDelegate?

    private let interactor: WarrenWalletInteractor
    private var hostingController: UIHostingController<AnyView>?
    private var isSaving = false

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
            "Restore wallet",
            tableName: "Wallet",
            comment: "Title shown above the mnemonic input grid"
        )

        navigationItem.leftBarButtonItem = UIBarButtonItem(
            barButtonSystemItem: .cancel,
            target: self,
            action: #selector(handleCancel)
        )

        installInputView()
    }

    private func installInputView() {
        let inputView = WarrenMnemonicInputView(
            onComplete: { [weak self] phrase in
                self?.handleSubmit(mnemonic: phrase)
            }
        )
        let hosting = UIHostingController(rootView: AnyView(inputView))
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

    private func handleSubmit(mnemonic: String) {
        guard !isSaving else { return }
        isSaving = true
        interactor.importMnemonic(mnemonic) { [weak self] result in
            guard let self else { return }
            self.isSaving = false
            switch result {
            case .success:
                self.delegate?.walletImportController(self, didImportMnemonic: mnemonic)
            case .failure(let error):
                self.presentErrorAlert(error)
            }
        }
    }

    private func presentErrorAlert(_ error: WarrenWalletInteractorError) {
        let message: String
        switch error {
        case .invalidMnemonic:
            message = NSLocalizedString(
                "The recovery phrase is invalid. Check each word against the BIP39 wordlist and try again.",
                tableName: "Wallet",
                comment: ""
            )
        case .keychain:
            message = NSLocalizedString(
                "Failed to securely store the wallet. Make sure your device is unlocked and try again.",
                tableName: "Wallet",
                comment: ""
            )
        case .authenticationFailed(let m):
            message = m
        default:
            message = NSLocalizedString(
                "An unexpected error occurred while restoring your wallet.",
                tableName: "Wallet",
                comment: ""
            )
        }
        let alert = UIAlertController(
            title: NSLocalizedString("Restore failed", tableName: "Wallet", comment: ""),
            message: message,
            preferredStyle: .alert
        )
        alert.addAction(
            UIAlertAction(
                title: NSLocalizedString("OK", tableName: "Wallet", comment: ""),
                style: .default
            )
        )
        present(alert, animated: true)
    }

    @objc private func handleCancel() {
        delegate?.walletImportControllerDidCancel(self)
    }
}
