//
//  AccountNumberRow.swift
//  MullvadVPN
//
//  Created by Mojgan on 2023-08-28.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import Foundation
import UIKit

/// Wallet identity row: the shortened SS58 address with a copy button,
/// mirroring the desktop pubkey row (no reveal toggle: the address is
/// public material, only the recovery phrase is secret). Copying always
/// places the FULL address on the pasteboard.
class AccountNumberRow: UIView {
    var accountNumber: String? {
        didSet {
            updateView()
        }
    }

    var copyAccountNumber: (() -> Void)?

    private let titleLabel: UILabel = {
        let textLabel = UILabel()
        // The Warren identity is the wallet public key; "account number"
        // is kept in parentheses so users coming from other VPNs still
        // recognize the field (mirrors the desktop account view label).
        textLabel.text = NSLocalizedString("Public key (account number)", comment: "")
        textLabel.font = .warrenTiny
        textLabel.adjustsFontForContentSizeCategory = true
        textLabel.textColor = UIColor(white: 1.0, alpha: 0.6)
        return textLabel
    }()

    private let accountNumberLabel: UILabel = {
        let textLabel = UILabel()
        textLabel.font = .warrenSmall
        textLabel.adjustsFontForContentSizeCategory = true
        textLabel.textColor = .white
        textLabel.numberOfLines = 0
        return textLabel
    }()

    private let copyButton: UIButton = {
        let button = UIButton(type: .system)
        button.adjustsImageSizeForAccessibilityContentSizeCategory = true
        button.tintColor = .white
        button.setContentHuggingPriority(.defaultHigh, for: .horizontal)
        return button
    }()

    private var revertCopyImageWorkItem: DispatchWorkItem?

    override init(frame: CGRect) {
        super.init(frame: frame)

        addConstrainedSubviews([titleLabel, accountNumberLabel, copyButton]) {
            titleLabel.pinEdgesToSuperview(.all().excluding([.trailing, .bottom]))
            titleLabel.trailingAnchor.constraint(greaterThanOrEqualTo: trailingAnchor)

            accountNumberLabel.topAnchor.constraint(equalTo: titleLabel.bottomAnchor, constant: UIMetrics.padding8)
            accountNumberLabel.leadingAnchor.constraint(equalTo: leadingAnchor)
            accountNumberLabel.trailingAnchor.constraint(greaterThanOrEqualTo: copyButton.leadingAnchor)
            accountNumberLabel.bottomAnchor.constraint(equalTo: bottomAnchor)

            copyButton.heightAnchor.constraint(equalTo: accountNumberLabel.heightAnchor)
            copyButton.centerYAnchor.constraint(equalTo: accountNumberLabel.centerYAnchor)
            copyButton.trailingAnchor.constraint(equalTo: trailingAnchor)
        }

        copyButton.addTarget(
            self,
            action: #selector(didTapCopyAccountNumber),
            for: .touchUpInside
        )

        isAccessibilityElement = true
        accessibilityLabel = titleLabel.text

        copyButton.setContentCompressionResistancePriority(.required, for: .horizontal)
        copyButton.setContentHuggingPriority(.required, for: .horizontal)

        accountNumberLabel.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        accountNumberLabel.setContentHuggingPriority(.defaultHigh, for: .horizontal)

        showCheckmark(false)
        updateView()
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    func setButtons(enabled: Bool) {
        copyButton.isEnabled = enabled
    }

    // MARK: - Private

    private func updateView() {
        accountNumberLabel.text = accountNumber?.shortWarrenAddress ?? ""

        accessibilityAttributedValue = _accessibilityAttributedValue
        accessibilityCustomActions = _accessibilityCustomActions
    }

    private var _accessibilityAttributedValue: NSAttributedString? {
        guard let accountNumber else {
            return nil
        }

        return NSAttributedString(
            string: accountNumber,
            attributes: [.accessibilitySpeechSpellOut: true]
        )
    }

    private var _accessibilityCustomActions: [UIAccessibilityCustomAction]? {
        guard accountNumber != nil else { return nil }

        return [
            UIAccessibilityCustomAction(
                name: NSLocalizedString("Copied Warren account number to pasteboard", comment: ""),
                target: self,
                selector: #selector(didTapCopyAccountNumber)
            )
        ]
    }

    private func showCheckmark(_ showCheckmark: Bool) {
        if showCheckmark {
            let tickIcon = UIImage.tick

            copyButton.setImage(tickIcon, for: .normal)
            copyButton.tintColor = .successColor
        } else {
            let copyIcon = UIImage.Buttons.copy

            copyButton.setImage(copyIcon, for: .normal)
            copyButton.tintColor = .white
        }
    }

    // MARK: - Actions

    @objc private func didTapCopyAccountNumber() {
        let delayedWorkItem = DispatchWorkItem { [weak self] in
            self?.showCheckmark(false)
        }

        revertCopyImageWorkItem?.cancel()
        revertCopyImageWorkItem = delayedWorkItem

        showCheckmark(true)
        copyAccountNumber?()

        DispatchQueue.main.asyncAfter(
            deadline: .now() + .seconds(2),
            execute: delayedWorkItem
        )
    }
}
