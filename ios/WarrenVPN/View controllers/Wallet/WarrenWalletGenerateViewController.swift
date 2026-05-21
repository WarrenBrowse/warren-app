//
//  WarrenWalletGenerateViewController.swift
//  WarrenVPN
//
//  Created by Warren on 2026-05-21.
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  UIKit shell hosting `WarrenMnemonicDisplayView` (SwiftUI). Step 2a
//  of the onboarding wizard: generate a new BIP39 mnemonic, show it
//  blur-and-reveal, require user confirmation before persisting to the
//  Keychain.
//

import SwiftUI
import UIKit

protocol WarrenWalletGenerateViewControllerDelegate: AnyObject {
    /// Called after the user has confirmed they have safely written
    /// down the mnemonic. The mnemonic is freshly generated and not
    /// yet persisted: the delegate is responsible for triggering the
    /// `WarrenWalletInteractor.saveMnemonic` flow.
    func walletGenerateController(_ controller: WarrenWalletGenerateViewController, didConfirmMnemonic mnemonic: String)

    /// Called if the user backs out of the generate flow.
    func walletGenerateControllerDidCancel(_ controller: WarrenWalletGenerateViewController)
}

final class WarrenWalletGenerateViewController: UIViewController {
    weak var delegate: WarrenWalletGenerateViewControllerDelegate?

    private let interactor: WarrenWalletInteractor
    private var hostingController: UIHostingController<AnyView>?
    private var mnemonic: String?

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
            comment: "Title shown above the freshly generated mnemonic during onboarding"
        )

        navigationItem.leftBarButtonItem = UIBarButtonItem(
            barButtonSystemItem: .cancel,
            target: self,
            action: #selector(handleCancel)
        )

        view.addSubview(activityIndicator)
        NSLayoutConstraint.activate([
            activityIndicator.centerXAnchor.constraint(equalTo: view.centerXAnchor),
            activityIndicator.centerYAnchor.constraint(equalTo: view.centerYAnchor),
        ])

        startGeneration()
    }

    private func startGeneration() {
        activityIndicator.startAnimating()
        interactor.generateMnemonic { [weak self] result in
            guard let self else { return }
            self.activityIndicator.stopAnimating()
            switch result {
            case .success(let phrase):
                self.mnemonic = phrase
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
                guard let self, let phrase = self.mnemonic else { return }
                self.delegate?.walletGenerateController(self, didConfirmMnemonic: phrase)
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
        let alert = UIAlertController(
            title: NSLocalizedString(
                "Wallet creation failed",
                tableName: "Wallet",
                comment: "Alert title shown if BIP39 generation or Keychain save fails"
            ),
            message: descriptionFor(error),
            preferredStyle: .alert
        )
        alert.addAction(
            UIAlertAction(
                title: NSLocalizedString("Try again", tableName: "Wallet", comment: "Retry button"),
                style: .default,
                handler: { [weak self] _ in self?.startGeneration() }
            )
        )
        alert.addAction(
            UIAlertAction(
                title: NSLocalizedString("Cancel", tableName: "Wallet", comment: "Cancel button"),
                style: .cancel,
                handler: { [weak self] _ in
                    guard let self else { return }
                    self.delegate?.walletGenerateControllerDidCancel(self)
                }
            )
        )
        present(alert, animated: true)
    }

    private func descriptionFor(_ error: WarrenWalletInteractorError) -> String {
        switch error {
        case .invalidMnemonic:
            return NSLocalizedString("The recovery phrase is invalid.", tableName: "Wallet", comment: "")
        case .generationFailed:
            return NSLocalizedString("Failed to generate a new recovery phrase. Please try again.", tableName: "Wallet", comment: "")
        case .keychain:
            return NSLocalizedString("Failed to securely store the wallet. Make sure your device is unlocked and try again.", tableName: "Wallet", comment: "")
        case .authenticationFailed(let message), .keychain where false == true:
            return message
        case .noWallet:
            return NSLocalizedString("No wallet is currently provisioned on this device.", tableName: "Wallet", comment: "")
        default:
            return NSLocalizedString("An unexpected error occurred.", tableName: "Wallet", comment: "")
        }
    }

    @objc private func handleCancel() {
        delegate?.walletGenerateControllerDidCancel(self)
    }
}
