//
//  AccountContentView.swift
//  MullvadVPN
//
//  Created by pronebird on 08/07/2021.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import UIKit

/// Account screen layout mirroring the desktop account view: a
/// subscription card (remaining time prominent when active + paid-until
/// date), an identity card (wallet public key with copy), then the
/// actions in the desktop order and styles: buy more credit (green CTA),
/// redeem voucher, backup keys (outline navigation row), log out (red
/// ghost link). Delete account has no desktop equivalent but is required
/// on iOS by App Store review guideline 5.1.1(v).
class AccountContentView: UIView {
    let purchaseButton: InAppPurchaseButton = {
        let button = InAppPurchaseButton()
        button.setAccessibilityIdentifier(.purchaseButton)
        button.setTitle(NSLocalizedString("Buy more credit", comment: ""), for: .normal)
        return button
    }()

    let redeemVoucherButton: AppButton = {
        let button = AppButton(style: .default)
        button.setAccessibilityIdentifier(.redeemVoucherButton)
        button.setTitle(NSLocalizedString("Redeem voucher", comment: ""), for: .normal)
        return button
    }()

    let backupKeysButton: NavigationRowButton = {
        let button = NavigationRowButton(title: NSLocalizedString("Backup keys", comment: ""))
        button.setAccessibilityIdentifier(.backupKeysButton)
        return button
    }()

    let logoutButton: GhostDangerButton = {
        let button = GhostDangerButton(title: NSLocalizedString("Log out", comment: ""))
        button.setAccessibilityIdentifier(.logoutButton)
        return button
    }()

    let deleteButton: AppButton = {
        let button = AppButton(style: .translucentDanger)
        button.setAccessibilityIdentifier(.deleteButton)
        button.setTitle(NSLocalizedString("Delete account", comment: ""), for: .normal)
        return button
    }()

    /// Remaining subscription time, shown big so the single most useful
    /// fact on this screen is readable at a glance. Hidden when the
    /// subscription is expired: the paid-until row alone carries the red
    /// OUT OF TIME state (desktop behavior), never both at once.
    /// Internal (not private) so unit tests can observe the hide logic.
    let remainingTimeLabel: UILabel = {
        let label = UILabel()
        label.font = .warrenLarge
        label.adjustsFontForContentSizeCategory = true
        label.textColor = .successColor
        label.numberOfLines = 0
        return label
    }()

    let accountTokenRowView: AccountNumberRow = {
        AccountNumberRow()
    }()

    let accountExpiryRowView: AccountExpiryRow = {
        AccountExpiryRow()
    }()

    private lazy var subscriptionCardView: UIView = {
        let stackView = UIStackView(arrangedSubviews: [remainingTimeLabel, accountExpiryRowView])
        stackView.axis = .vertical
        stackView.spacing = UIMetrics.padding16
        return Self.makeCard(content: stackView)
    }()

    private lazy var identityCardView: UIView = {
        Self.makeCard(content: accountTokenRowView)
    }()

    lazy var contentStackView: UIStackView = {
        let stackView =
            UIStackView(arrangedSubviews: [
                subscriptionCardView,
                identityCardView,
            ])
        stackView.axis = .vertical
        stackView.spacing = UIMetrics.padding16
        return stackView
    }()

    lazy var buttonStackView: UIStackView = {
        let arrangedSubviews: [UIView] = [
            purchaseButton,
            redeemVoucherButton,
            backupKeysButton,
            logoutButton,
            deleteButton,
        ]
        arrangedSubviews.forEach { $0.isExclusiveTouch = true }
        let stackView = UIStackView(arrangedSubviews: arrangedSubviews)
        stackView.axis = .vertical
        stackView.spacing = UIMetrics.padding16
        return stackView
    }()

    override init(frame: CGRect) {
        super.init(frame: frame)
        setAccessibilityIdentifier(.accountView)
        addScrollView()
    }

    /// Updates both the prominent remaining-time headline and the
    /// paid-until row from the same expiry so they can never disagree.
    func setExpiry(_ expiry: Date?) {
        accountExpiryRowView.value = expiry

        if let expiry, expiry > Date() {
            remainingTimeLabel.text = CustomDateComponentsFormatting.localizedString(
                from: Date(),
                to: expiry,
                unitsStyle: .full
            )
            remainingTimeLabel.isHidden = false
        } else {
            remainingTimeLabel.text = nil
            remainingTimeLabel.isHidden = true
        }
    }

    /// Surface card lifting the account facts off the flat background,
    /// matching the desktop account view cards.
    private static func makeCard(content: UIView) -> UIView {
        let card = UIView()
        card.backgroundColor = .primaryColor
        card.layer.cornerRadius = UIMetrics.padding16
        card.addConstrainedSubviews([content]) {
            content.pinEdgesToSuperview(
                .all(
                    NSDirectionalEdgeInsets(
                        top: UIMetrics.padding16,
                        leading: UIMetrics.padding16,
                        bottom: UIMetrics.padding16,
                        trailing: UIMetrics.padding16
                    )))
        }
        return card
    }

    private func addScrollView() {
        let scrollView = UIScrollView()
        let contentView = UIView()

        addConstrainedSubviews([scrollView]) {
            scrollView.pinEdgesToSuperviewMargins()
        }

        scrollView.addConstrainedSubviews([contentView]) {
            contentView.pinEdgesToSuperview()
            contentView.widthAnchor.constraint(equalTo: scrollView.widthAnchor)
            contentView.heightAnchor.constraint(greaterThanOrEqualTo: scrollView.frameLayoutGuide.heightAnchor)
        }

        let spacer = UIView()

        contentView.addConstrainedSubviews([contentStackView, spacer, buttonStackView]) {
            contentStackView.pinEdgesToSuperviewMargins(.all().excluding(.bottom))
            spacer.pinEdgesToSuperviewMargins(.all().excluding(.top).excluding(.bottom))
            buttonStackView.pinEdgesToSuperviewMargins(.all().excluding(.top))

            spacer.bottomAnchor.constraint(equalTo: buttonStackView.topAnchor)
            spacer.topAnchor.constraint(
                equalTo: contentStackView.bottomAnchor,
                constant: UIMetrics.TableView.sectionSpacing
            )
        }
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }
}

/// Outline navigation row matching the desktop "Backup keys" row: a
/// transparent bordered row with a leading title and a trailing chevron.
/// Deliberately quieter than the colored action buttons: it navigates, it
/// does not act.
final class NavigationRowButton: UIControl {
    private let titleLabel: UILabel = {
        let label = UILabel()
        label.font = .warrenSmallSemiBold
        label.adjustsFontForContentSizeCategory = true
        label.textColor = .white
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

    init(title: String) {
        super.init(frame: .zero)

        titleLabel.text = title
        layer.cornerRadius = UIMetrics.controlCornerRadius
        layer.borderWidth = 1
        layer.borderColor = UIColor(white: 1.0, alpha: 0.2).cgColor

        addConstrainedSubviews([titleLabel, chevronView]) {
            titleLabel.pinEdgesToSuperview(
                .init([.top(12), .bottom(12), .leading(UIMetrics.padding16)]))
            chevronView.centerYAnchor.constraint(equalTo: centerYAnchor)
            chevronView.leadingAnchor.constraint(
                greaterThanOrEqualTo: titleLabel.trailingAnchor,
                constant: UIMetrics.padding8
            )
            chevronView.trailingAnchor.constraint(
                equalTo: trailingAnchor,
                constant: -UIMetrics.padding16
            )
        }

        isAccessibilityElement = true
        accessibilityLabel = title
        accessibilityTraits = .button
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

/// Red ghost link matching the desktop "Log out" button: plain red text,
/// no background, sitting at the very bottom of the action stack.
final class GhostDangerButton: UIButton {
    init(title: String) {
        super.init(frame: .zero)

        var config = UIButton.Configuration.plain()
        config.attributedTitle = AttributedString(
            title,
            attributes: AttributeContainer([.font: UIFont.warrenSmallSemiBold])
        )
        config.baseForegroundColor = .dangerColor
        config.contentInsets = NSDirectionalEdgeInsets(top: 12, leading: 8, bottom: 12, trailing: 8)
        configuration = config
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }
}
