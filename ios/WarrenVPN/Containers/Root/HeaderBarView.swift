//
//  HeaderBarView.swift
//  MullvadVPN
//
//  Created by pronebird on 19/06/2020.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import UIKit

class HeaderBarView: UIView {
    // One-piece "Warren" lockup (the desktop wordmark SVG): the W with the
    // ears IS the first letter of the word, so the mark and the name can
    // never drift apart. Never split it back into logo + text images.
    private let wordmarkImageView: UIImageView = {
        let imageView = UIImageView(image: UIImage(named: "WarrenWordmark"))
        imageView.contentMode = .scaleAspectFit
        return imageView
    }()

    private let deviceInfoHolder: UIStackView = {
        let stackView = UIStackView()
        stackView.axis = .horizontal
        stackView.distribution = .fill
        stackView.spacing = 8.0
        return stackView
    }()

    private lazy var deviceNameLabel: UILabel = {
        let label = UILabel()
        label.font = .warrenMiniSemiBold
        label.adjustsFontForContentSizeCategory = true
        label.textColor = UIColor(white: 1.0, alpha: 0.8)
        label.setContentHuggingPriority(.defaultHigh, for: .horizontal)  // Resist growing
        label.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        label.setAccessibilityIdentifier(.headerDeviceNameLabel)
        return label
    }()

    private lazy var timeLeftLabel: UILabel = {
        let label = UILabel()
        label.font = .warrenMiniSemiBold
        label.adjustsFontForContentSizeCategory = true
        label.textColor = UIColor(white: 1.0, alpha: 0.8)
        label.setContentHuggingPriority(.defaultLow, for: .horizontal)  // Allow growing
        label.setContentCompressionResistancePriority(.required, for: .horizontal)
        return label
    }()

    private lazy var buttonContainer: UIStackView = {
        let stackView = UIStackView(arrangedSubviews: [accountButton, settingsButton])
        stackView.spacing = 12
        return stackView
    }()

    private let borderLayer: CALayer = {
        let layer = CALayer()
        layer.backgroundColor = UIColor.HeaderBar.dividerColor.cgColor
        return layer
    }()

    private let breadcrumbImageView: UIImageView = {
        let imageView = UIImageView()
        imageView.widthAnchor.constraint(equalToConstant: 18).isActive = true
        imageView.heightAnchor.constraint(equalTo: imageView.widthAnchor).isActive = true
        return imageView
    }()

    let accountButton: UIButton = {
        let button = makeHeaderBarButton(with: UIImage.Buttons.account)
        button.setAccessibilityIdentifier(.accountButton)
        button.accessibilityLabel = NSLocalizedString("Account", comment: "")
        button.heightAnchor.constraint(equalToConstant: UIMetrics.Button.barButtonSize).isActive = true
        button.widthAnchor.constraint(equalTo: button.heightAnchor, multiplier: 1).isActive = true
        return button
    }()

    let settingsButton: UIButton = {
        let button = makeHeaderBarButton(with: UIImage.Buttons.settings)
        button.setAccessibilityIdentifier(.settingsButton)
        button.accessibilityLabel = NSLocalizedString("Settings", comment: "")
        button.heightAnchor.constraint(equalToConstant: UIMetrics.Button.barButtonSize).isActive = true
        button.widthAnchor.constraint(equalTo: button.heightAnchor, multiplier: 1).isActive = true
        return button
    }()

    class func makeHeaderBarButton(with image: UIImage?) -> IncreasedHitButton {
        let buttonImage = image?.withTintColor(UIColor.HeaderBar.buttonColor, renderingMode: .alwaysOriginal)
        let barButton = IncreasedHitButton(type: .system)
        // setImage, not setBackgroundImage: the outline glyphs render at their
        // intrinsic size centered in the (larger) tappable button instead of
        // being stretched to fill it, which would fatten their strokes.
        barButton.setImage(buttonImage, for: .normal)
        barButton.configureForAutoLayout()

        return barButton
    }

    /// Light = white content over the colored/charcoal bars; dark = black
    /// content over the transparent bar riding the bright scenery sky
    /// (desktop main header tone).
    var tone: HeaderBarTone = .light {
        didSet {
            guard tone != oldValue else { return }
            applyTone()
        }
    }

    private var contentColor: UIColor {
        switch tone {
        case .light: UIColor.HeaderBar.buttonColor
        case .dark: UIColor.black
        }
    }

    private var subduedContentColor: UIColor {
        switch tone {
        case .light: UIColor(white: 1.0, alpha: 0.8)
        case .dark: UIColor(white: 0.0, alpha: 0.8)
        }
    }

    private func applyTone() {
        wordmarkImageView.image = UIImage(named: "WarrenWordmark")?
            .withTintColor(contentColor, renderingMode: .alwaysOriginal)
        accountButton.setImage(
            UIImage.Buttons.account.withTintColor(contentColor, renderingMode: .alwaysOriginal),
            for: .normal
        )
        let settingsImage = breadcrumb != nil ? UIImage.Buttons.settingsPartial : UIImage.Buttons.settings
        settingsButton.setImage(
            settingsImage.withTintColor(contentColor, renderingMode: .alwaysOriginal),
            for: .normal
        )
        deviceNameLabel.textColor = subduedContentColor
        timeLeftLabel.textColor = subduedContentColor
    }

    var showsDivider = false {
        didSet {
            if showsDivider {
                layer.addSublayer(borderLayer)
            } else {
                borderLayer.removeFromSuperlayer()
            }
        }
    }

    var isDeviceInfoHidden = false {
        didSet {
            deviceInfoHolder.arrangedSubviews.forEach { $0.isHidden = isDeviceInfoHidden }
        }
    }

    var breadcrumb: Breadcrumb? {
        didSet {
            if let breadcrumb {
                breadcrumbImageView.image = breadcrumb.icon
                breadcrumbImageView.isHidden = false
                settingsButton.setImage(
                    .Buttons.settingsPartial.withTintColor(contentColor, renderingMode: .alwaysOriginal),
                    for: .normal
                )
            } else {
                breadcrumbImageView.isHidden = true
                settingsButton.setImage(
                    .Buttons.settings.withTintColor(contentColor, renderingMode: .alwaysOriginal),
                    for: .normal
                )
            }
        }
    }

    private var isAccountButtonHidden = false {
        didSet {
            accountButton.isHidden = isAccountButtonHidden
        }
    }

    private var timeLeft: Date? {
        didSet {
            if let timeLeft {
                let formattedTimeLeft = NSLocalizedString("Time left: %@", comment: "")
                timeLeftLabel.text = String(
                    format: formattedTimeLeft,
                    CustomDateComponentsFormatting.localizedString(
                        from: Date(),
                        to: timeLeft,
                        unitsStyle: .full
                    ) ?? ""
                )
            } else {
                timeLeftLabel.text = ""
            }
        }
    }

    private var deviceName: String? {
        didSet {
            if let deviceName {
                let formattedDeviceName = NSLocalizedString("Device name: %@", comment: "")
                deviceNameLabel.text = String(format: formattedDeviceName, deviceName)
            } else {
                deviceNameLabel.text = ""
            }
        }
    }

    override init(frame: CGRect) {
        super.init(frame: frame)
        directionalLayoutMargins = NSDirectionalEdgeInsets(
            top: 0,
            leading: 16,
            bottom: 0,
            trailing: 16
        )

        accessibilityContainerType = .semanticGroup
        setAccessibilityIdentifier(.headerBarView)

        let wordmarkSize = wordmarkImageView.image?.size ?? .zero
        let wordmarkAspectRatio = wordmarkSize.width / max(wordmarkSize.height, 1)

        var buttonContainerTrailingAdjustment: CGFloat = 0
        if let buttonImageWidth = settingsButton.currentImage?.size.width {
            buttonContainerTrailingAdjustment = max((UIMetrics.Button.barButtonSize - buttonImageWidth) / 2, 0)
        }

        settingsButton.addConstrainedSubviews([breadcrumbImageView]) {
            breadcrumbImageView.pinEdgesToSuperview(.init([.top(-3), .trailing(-3)]))
        }

        [deviceNameLabel, timeLeftLabel].forEach { deviceInfoHolder.addArrangedSubview($0) }

        addConstrainedSubviews([wordmarkImageView, buttonContainer, deviceInfoHolder]) {
            wordmarkImageView.leadingAnchor.constraint(equalTo: layoutMarginsGuide.leadingAnchor)
            wordmarkImageView.topAnchor.constraint(
                equalTo: layoutMarginsGuide.topAnchor,
                constant: 8
            )
            wordmarkImageView.heightAnchor.constraint(equalToConstant: UIMetrics.headerBarLockupHeight)
            wordmarkImageView.widthAnchor.constraint(
                equalTo: wordmarkImageView.heightAnchor,
                multiplier: wordmarkAspectRatio
            )

            // The lockup's letters sit in the bottom band of its box (the W's
            // ears tower above), so the buttons center on the letter band, the
            // optical alignment the eye expects, not on the box middle.
            buttonContainer.centerYAnchor.constraint(
                equalTo: wordmarkImageView.centerYAnchor,
                constant: 5
            )
            buttonContainer.trailingAnchor.constraint(
                equalTo: layoutMarginsGuide.trailingAnchor,
                constant: buttonContainerTrailingAdjustment
            )

            deviceInfoHolder.leadingAnchor.constraint(equalTo: layoutMarginsGuide.leadingAnchor)
            deviceInfoHolder.trailingAnchor.constraint(equalTo: layoutMarginsGuide.trailingAnchor)
            deviceInfoHolder.topAnchor.constraint(
                equalToSystemSpacingBelow: wordmarkImageView.bottomAnchor,
                multiplier: 1
            )
            layoutMarginsGuide.bottomAnchor.constraint(
                equalToSystemSpacingBelow: deviceInfoHolder.bottomAnchor,
                multiplier: 1
            )
        }

        applyTone()
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override func layoutSubviews() {
        super.layoutSubviews()

        borderLayer.frame = CGRect(x: 0, y: frame.maxY - 1, width: frame.width, height: 1)
    }

    func update(configuration: RootConfiguration) {
        deviceName = configuration.deviceName
        timeLeft = configuration.expiry
        isAccountButtonHidden = !configuration.showsAccountButton
    }
}
