//
//  AccountExpiryRow.swift
//  MullvadVPN
//
//  Created by Mojgan on 2023-08-28.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import Foundation
import UIKit

class AccountExpiryRow: UIView {
    var value: Date? {
        didSet {
            let expiry = value

            if let expiry, expiry <= Date() {
                let localizedString = NSLocalizedString("OUT OF TIME", comment: "")

                valueLabel.text = localizedString
                accessibilityValue = localizedString

                valueLabel.textColor = .dangerColor
            } else if let expiry {
                let formattedDate = DateFormatter.localizedString(
                    from: expiry,
                    dateStyle: .medium,
                    timeStyle: .short
                )

                valueLabel.text = formattedDate
                accessibilityValue = formattedDate

                valueLabel.textColor = .white
            } else {
                // No expiry data yet (fetch pending or failed): desktop
                // shows an explicit placeholder instead of an empty row.
                let localizedString = NSLocalizedString("Currently unavailable", comment: "")

                valueLabel.text = localizedString
                accessibilityValue = localizedString

                valueLabel.textColor = .white
            }
        }
    }

    private let textLabel: UILabel = {
        let textLabel = UILabel()
        textLabel.text = NSLocalizedString("Paid until", comment: "")
        textLabel.font = .warrenTiny
        textLabel.numberOfLines = 0
        textLabel.adjustsFontForContentSizeCategory = true
        textLabel.textColor = UIColor(white: 1.0, alpha: 0.6)
        textLabel.setContentCompressionResistancePriority(.required, for: .horizontal)
        textLabel.setContentHuggingPriority(.defaultHigh, for: .horizontal)
        return textLabel
    }()

    private let valueLabel: UILabel = {
        let valueLabel = UILabel()
        valueLabel.translatesAutoresizingMaskIntoConstraints = false
        valueLabel.font = .warrenSmall
        valueLabel.adjustsFontForContentSizeCategory = true
        valueLabel.textColor = .white
        valueLabel.numberOfLines = 0
        valueLabel.setAccessibilityIdentifier(.accountPagePaidUntilLabel)
        return valueLabel
    }()

    let activityIndicator: SpinnerActivityIndicatorView = {
        let activityIndicator = SpinnerActivityIndicatorView(style: .small)
        activityIndicator.translatesAutoresizingMaskIntoConstraints = false
        activityIndicator.tintColor = .white
        activityIndicator.setContentHuggingPriority(.defaultHigh, for: .horizontal)
        activityIndicator.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        activityIndicator.setContentHuggingPriority(.defaultLow, for: .horizontal)
        return activityIndicator
    }()

    override init(frame: CGRect) {
        super.init(frame: frame)
        let stackView: UIStackView = {
            let stackView = UIStackView(arrangedSubviews: [textLabel, activityIndicator, UIView()])
            stackView.axis = .horizontal
            stackView.distribution = .fill
            stackView.spacing = UIMetrics.padding8
            return stackView
        }()

        addConstrainedSubviews([stackView, valueLabel]) {
            stackView.pinEdgesToSuperview(.all().excluding([.bottom]))
            valueLabel.pinEdgesToSuperview(.all().excluding(.top))
            valueLabel.topAnchor.constraint(equalTo: textLabel.bottomAnchor, constant: UIMetrics.padding8)
        }
        isAccessibilityElement = true
        accessibilityLabel = textLabel.text
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }
}
