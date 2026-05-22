//
//  WarrenWalletEraseViewController.swift
//  WarrenVPN
//
//  Created by Warren on 2026-05-22 (C.6 follow-up).
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  Settings → "Erase wallet" destructive action. Wipes the wallet from
//  the iOS Keychain (`WarrenWalletKeychain.delete()`), zeroes any
//  in-memory copy, and dismisses back to Settings. Gated by a
//  confirmation alert because the operation is irreversible without
//  the backup mnemonic.
//

import UIKit
import WarrenLogging

protocol WarrenWalletEraseViewControllerDelegate: AnyObject {
    /// Called when the user has confirmed the wipe and the Keychain
    /// entry has been deleted. Caller is responsible for transitioning
    /// the user back to the onboarding wizard (`hasCompletedWarrenOnboarding`
    /// should be reset).
    func walletEraseControllerDidWipe(_ controller: WarrenWalletEraseViewController)
}

final class WarrenWalletEraseViewController: UIViewController {
    weak var delegate: WarrenWalletEraseViewControllerDelegate?

    private let logger = Logger(label: "WarrenWalletEraseViewController")

    private let stackView: UIStackView = {
        let stack = UIStackView()
        stack.axis = .vertical
        stack.spacing = 16
        stack.alignment = .fill
        stack.translatesAutoresizingMaskIntoConstraints = false
        return stack
    }()

    private let titleLabel: UILabel = {
        let label = UILabel()
        label.text = NSLocalizedString(
            "Erase Warren wallet",
            tableName: "Wallet",
            comment: "Title of the destructive wallet wipe screen"
        )
        label.font = .systemFont(ofSize: 22, weight: .semibold)
        label.textColor = .white
        label.numberOfLines = 0
        return label
    }()

    private let warningLabel: UILabel = {
        let label = UILabel()
        label.text = NSLocalizedString(
            "This will permanently delete your wallet from this device. You cannot recover access without your 12-word backup phrase. Continue only if you have safely stored the phrase elsewhere.",
            tableName: "Wallet",
            comment: "Multi-line warning before the destructive Erase wallet action"
        )
        label.font = .systemFont(ofSize: 15)
        label.textColor = .white.withAlphaComponent(0.75)
        label.numberOfLines = 0
        return label
    }()

    private lazy var eraseButton: UIButton = {
        let button = UIButton(type: .system)
        button.setTitle(
            NSLocalizedString(
                "Erase wallet",
                tableName: "Wallet",
                comment: "Confirmation button label on the destructive Erase wallet screen"
            ),
            for: .normal
        )
        button.setTitleColor(.white, for: .normal)
        button.backgroundColor = .Warren.error
        button.titleLabel?.font = .systemFont(ofSize: 17, weight: .semibold)
        button.layer.cornerRadius = 8
        button.heightAnchor.constraint(equalToConstant: 48).isActive = true
        button.addTarget(self, action: #selector(handleEraseTapped), for: .touchUpInside)
        button.translatesAutoresizingMaskIntoConstraints = false
        return button
    }()

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .Warren.navy
        title = NSLocalizedString(
            "Erase wallet",
            tableName: "Wallet",
            comment: "Navigation title on the Erase wallet screen"
        )

        view.addSubview(stackView)
        stackView.addArrangedSubview(titleLabel)
        stackView.addArrangedSubview(warningLabel)
        stackView.addArrangedSubview(eraseButton)
        NSLayoutConstraint.activate([
            stackView.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor, constant: 24),
            stackView.leadingAnchor.constraint(equalTo: view.layoutMarginsGuide.leadingAnchor),
            stackView.trailingAnchor.constraint(equalTo: view.layoutMarginsGuide.trailingAnchor),
        ])
    }

    @objc private func handleEraseTapped() {
        let alert = UIAlertController(
            title: NSLocalizedString(
                "Erase wallet?",
                tableName: "Wallet",
                comment: "Destructive confirmation alert title"
            ),
            message: NSLocalizedString(
                "This action cannot be undone. Make sure your 12-word backup phrase is recorded somewhere safe.",
                tableName: "Wallet",
                comment: "Destructive confirmation alert body"
            ),
            preferredStyle: .alert
        )
        alert.addAction(
            UIAlertAction(
                title: NSLocalizedString("Cancel", tableName: "Wallet", comment: ""),
                style: .cancel
            )
        )
        alert.addAction(
            UIAlertAction(
                title: NSLocalizedString(
                    "Erase",
                    tableName: "Wallet",
                    comment: "Destructive confirmation button"
                ),
                style: .destructive,
                handler: { [weak self] _ in
                    self?.performWipe()
                }
            )
        )
        present(alert, animated: true)
    }

    private func performWipe() {
        do {
            try WarrenWalletKeychain.delete()
            logger.info("Warren wallet wiped from Keychain")
            if let delegate {
                delegate.walletEraseControllerDidWipe(self)
            } else if let nav = navigationController {
                // Fallback when no coordinator delegate is wired :
                // pop back to Settings. The Settings table reloads
                // on `viewWillAppear`, dropping the now-irrelevant
                // wallet rows (`WarrenWalletKeychain.exists()` returns
                // false after this point).
                nav.popViewController(animated: true)
            } else {
                dismiss(animated: true)
            }
        } catch {
            logger.error("Failed to erase wallet: \(error)")
            let alert = UIAlertController(
                title: NSLocalizedString(
                    "Cannot erase wallet",
                    tableName: "Wallet",
                    comment: ""
                ),
                message: error.localizedDescription,
                preferredStyle: .alert
            )
            alert.addAction(
                UIAlertAction(
                    title: NSLocalizedString("Close", tableName: "Wallet", comment: ""),
                    style: .default
                )
            )
            present(alert, animated: true)
        }
    }
}
