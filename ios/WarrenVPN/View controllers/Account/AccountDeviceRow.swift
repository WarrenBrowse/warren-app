//
//  AccountDeviceRow.swift
//  MullvadVPN
//
//  Created by Mojgan on 2023-08-28.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import UIKit

class AccountDeviceRow: UIView {
    var deviceName: String? {
        didSet {
            deviceLabel.text = deviceName?.capitalized ?? ""
            accessibilityValue = deviceName
            setAccessibilityIdentifier(.accountPageDeviceNameLabel)
        }
    }

    private let titleLabel: UILabel = {
        let label = UILabel()
        label.text = NSLocalizedString("Device name", comment: "")
        label.font = .warrenTiny
        label.adjustsFontForContentSizeCategory = true
        label.textColor = UIColor(white: 1.0, alpha: 0.6)
        return label
    }()

    private let deviceLabel: UILabel = {
        let label = UILabel()
        label.font = .warrenSmall
        label.adjustsFontForContentSizeCategory = true
        label.textColor = .white
        label.numberOfLines = 0
        return label
    }()

    override init(frame: CGRect) {
        super.init(frame: frame)

        let contentContainerView = UIStackView(arrangedSubviews: [titleLabel, deviceLabel])
        contentContainerView.axis = .vertical
        contentContainerView.alignment = .leading
        contentContainerView.spacing = 8

        contentContainerView.setContentCompressionResistancePriority(.required, for: .horizontal)
        contentContainerView.setContentHuggingPriority(.defaultHigh, for: .horizontal)

        addConstrainedSubviews([contentContainerView]) {
            contentContainerView.pinEdgesToSuperview()
        }

        isAccessibilityElement = true
        accessibilityLabel = titleLabel.text
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }
}
