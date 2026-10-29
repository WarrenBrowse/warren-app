//
//  AccountDeviceRow.swift
//  MullvadVPN
//
//  Created by Mojgan on 2023-08-28.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import UIKit

/// The Account screen's "Devices" row (warren-core doc 115): the count of the
/// account's active devices, opening the list. Styled as the backup row since
/// both navigate. No count while the list is unknown: the list says why.
final class AccountDeviceRow: UIControl {
    var activeDeviceCount: Int? {
        didSet {
            countLabel.text = activeDeviceCount.map(String.init)
            countLabel.isHidden = activeDeviceCount == nil
            accessibilityValue = countLabel.text
        }
    }

    private let titleLabel: UILabel = {
        let label = UILabel()
        label.text = NSLocalizedString("Devices", comment: "")
        label.font = .warrenSmallSemiBold
        label.adjustsFontForContentSizeCategory = true
        label.textColor = .white
        return label
    }()

    private let countLabel: UILabel = {
        let label = UILabel()
        label.font = .warrenSmallSemiBold
        label.adjustsFontForContentSizeCategory = true
        label.textColor = UIColor(white: 1.0, alpha: 0.6)
        label.isHidden = true
        label.setContentHuggingPriority(.required, for: .horizontal)
        return label
    }()

    private let chevronView: UIImageView = {
        let imageView = UIImageView(
            image: UIImage.CellDecoration.chevronRight.withRenderingMode(.alwaysTemplate))
        imageView.tintColor = UIColor(white: 1.0, alpha: 0.6)
        imageView.setContentHuggingPriority(.required, for: .horizontal)
        imageView.setContentCompressionResistancePriority(.required, for: .horizontal)
        return imageView
    }()

    override init(frame: CGRect) {
        super.init(frame: frame)

        layer.cornerRadius = UIMetrics.controlCornerRadius
        layer.borderWidth = 1
        layer.borderColor = UIColor(white: 1.0, alpha: 0.2).cgColor

        addConstrainedSubviews([titleLabel, countLabel, chevronView]) {
            titleLabel.pinEdgesToSuperview(.init([.top(12), .bottom(12), .leading(UIMetrics.padding16)]))
            countLabel.centerYAnchor.constraint(equalTo: centerYAnchor)
            countLabel.leadingAnchor.constraint(
                greaterThanOrEqualTo: titleLabel.trailingAnchor,
                constant: UIMetrics.padding8
            )
            chevronView.centerYAnchor.constraint(equalTo: centerYAnchor)
            chevronView.leadingAnchor.constraint(equalTo: countLabel.trailingAnchor, constant: UIMetrics.padding8)
            chevronView.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -UIMetrics.padding16)
        }

        isAccessibilityElement = true
        accessibilityLabel = titleLabel.text
        accessibilityTraits = .button
        setAccessibilityIdentifier(.deviceManagementButton)
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override var isHighlighted: Bool {
        didSet {
            backgroundColor = isHighlighted ? UIColor(white: 1.0, alpha: 0.2) : .clear
        }
    }

    override var isEnabled: Bool {
        didSet {
            alpha = isEnabled ? 1 : 0.5
        }
    }
}
