//
//  ForumHandleRow.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import UIKit

/// The anonymous name this wallet posts under on the community forum, learnt
/// from an approved sign-in. Mirrors the desktop `ForumHandleRow` and the
/// Android "Forum name" card: the handle is public on the forum, so it is
/// shown in clear; its link to the wallet is what stays on this device.
final class ForumHandleRow: UIView {
    var handle: String? {
        didSet {
            handleLabel.text = handle ?? ""
            accessibilityValue = handle
        }
    }

    private let titleLabel: UILabel = {
        let label = UILabel()
        label.text = NSLocalizedString("Forum name", comment: "Account row label: the anonymous forum handle")
        label.font = .warrenTiny
        label.adjustsFontForContentSizeCategory = true
        label.textColor = UIColor(white: 1.0, alpha: 0.6)
        return label
    }()

    private let handleLabel: UILabel = {
        let label = UILabel()
        label.font = .warrenSmall
        label.adjustsFontForContentSizeCategory = true
        label.textColor = .white
        label.numberOfLines = 0
        return label
    }()

    override init(frame: CGRect) {
        super.init(frame: frame)

        addConstrainedSubviews([titleLabel, handleLabel]) {
            titleLabel.pinEdgesToSuperview(.all().excluding(.bottom))
            handleLabel.topAnchor.constraint(equalTo: titleLabel.bottomAnchor, constant: UIMetrics.padding8)
            handleLabel.pinEdgesToSuperview(.all().excluding(.top))
        }

        isAccessibilityElement = true
        accessibilityLabel = titleLabel.text
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }
}
