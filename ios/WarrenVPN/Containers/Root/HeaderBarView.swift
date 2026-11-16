//
//  HeaderBarView.swift
//  MullvadVPN
//
//  Created by pronebird on 19/06/2020.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import UIKit
import WarrenRustRuntime

/// A label that reserves room around its text, so the product chip reads as a
/// pill around the word rather than a fill tight on its glyphs.
private final class InsetLabel: UILabel {
    var insets = UIEdgeInsets(top: 2, left: 6, bottom: 2, right: 6)

    override func drawText(in rect: CGRect) {
        super.drawText(in: rect.inset(by: insets))
    }

    override var intrinsicContentSize: CGSize {
        let size = super.intrinsicContentSize
        return CGSize(
            width: size.width + insets.left + insets.right,
            height: size.height + insets.top + insets.bottom
        )
    }
}

class HeaderBarView: UIView {
    // One-piece "Warren" lockup (the desktop wordmark SVG): the W with the
    // ears IS the first letter of the word, so the mark and the name can
    // never drift apart. Never split it back into logo + text images.
    private let wordmarkImageView: UIImageView = {
        let imageView = UIImageView(image: UIImage(named: "WarrenWordmark"))
        imageView.contentMode = .scaleAspectFit
        return imageView
    }()

    /// A paper copy of the wordmark under it, whose shadow is the wide halo
    /// that separates the ink from whatever the sky holds. Home only.
    private let wordmarkGlowView: UIImageView = {
        let imageView = UIImageView(image: UIImage(named: "WarrenWordmark"))
        imageView.contentMode = .scaleAspectFit
        imageView.isHidden = true
        imageView.layer.shadowOffset = .zero
        imageView.layer.shadowRadius = 7
        imageView.layer.shadowOpacity = 0.85
        return imageView
    }()

    /// The rounded translucent pill the home toolbar's buttons sit on.
    private let toolbarBackground = HomeToolbarBackgroundView()

    private var classicConstraints: [NSLayoutConstraint] = []
    private var homeConstraints: [NSLayoutConstraint] = []

    /// The theme of the home screen while the header floats over its art, nil
    /// on every other screen.
    var homeTheme: WarrenTheme? {
        didSet {
            guard homeTheme != oldValue else { return }
            applyLayout(home: homeTheme != nil, wasHome: oldValue != nil)
            applyTone()
        }
    }

    /// The non-prod marker, beside the wordmark. Once the app is open the home
    /// screen icon is out of sight and the status-bar VPN chip is the system's
    /// own, so this is the in-app answer to "which of the two installs am I
    /// looking at". Prod shows nothing: its header must not change.
    let productChipLabel: UILabel = {
        let label = InsetLabel()
        label.font = .warrenMiniSemiBold
        label.adjustsFontForContentSizeCategory = true
        label.textAlignment = .center
        label.layer.cornerRadius = 4
        label.layer.borderWidth = 1
        label.layer.masksToBounds = true
        label.setContentHuggingPriority(.required, for: .horizontal)
        label.setContentCompressionResistancePriority(.required, for: .horizontal)
        return label
    }()

    /// The marker text, `nil` on prod. Read from the compiled product table at
    /// init; the tests set it to exercise a build they are not running as.
    var productBadge: String? = WarrenProductAnchors.current.environmentBadge {
        didSet { applyProductBadge() }
    }

    /// Set while the screen below carries the marker itself (the connect
    /// screen's banner), so the build is not named twice.
    var isProductChipSuppressed = false {
        didSet { applyProductBadge() }
    }

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
        let stackView = UIStackView(arrangedSubviews: [forumButton, accountButton, settingsButton])
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

    /// The header's forum slot: the activity bell for a wallet that has a
    /// forum account, the lifebuoy into the forum for one that has not, and
    /// nothing at all while the setting is off. The desktop header's third
    /// button and Android's, on the same rule
    /// (`fixtures/client-rules/forum_activity.json`).
    let forumButton: UIButton = {
        let button = IncreasedHitButton(type: .system)
        button.configureForAutoLayout()
        button.heightAnchor.constraint(equalToConstant: UIMetrics.Button.barButtonSize).isActive = true
        button.widthAnchor.constraint(equalTo: button.heightAnchor, multiplier: 1).isActive = true
        button.isHidden = true
        return button
    }()

    /// The unread count over the bell. Hidden at zero: an empty badge would
    /// invite a click into a panel with nothing in it.
    let forumBadgeLabel: UILabel = {
        let label = InsetLabel()
        label.insets = UIEdgeInsets(top: 1, left: 4, bottom: 1, right: 4)
        label.font = .warrenMiniSemiBold
        label.adjustsFontForContentSizeCategory = true
        label.textAlignment = .center
        label.backgroundColor = UIColor.Warren.yellow
        label.textColor = UIColor.Warren.navy
        label.layer.cornerRadius = 7
        label.layer.masksToBounds = true
        label.isHidden = true
        // The button already carries the count in its accessibility label, so
        // a second reading of the same number would only repeat it.
        label.isAccessibilityElement = false
        return label
    }()

    /// Which button the forum slot carries. Set from the same pair the other
    /// two clients read: whether this wallet has a forum account, and whether
    /// the forum notifications setting is on.
    var forumSlot: WarrenForumHeaderButton = .none {
        didSet { applyForumSlot() }
    }

    /// Unread forum notifications for this installation, as the monitor
    /// publishes them.
    var forumUnread: Int = 0 {
        didSet { applyForumSlot() }
    }

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
        if let homeTheme {
            return ConnectSurfacePalette(theme: homeTheme).uiColor(.homeText)
        }
        return switch tone {
        case .light: UIColor.HeaderBar.buttonColor
        case .dark: UIColor.black
        }
    }

    /// The wordmark is inked like the art, so on the home it keeps its dark ink
    /// in both themes and rides its paper glow.
    private var wordmarkColor: UIColor {
        homeTheme != nil ? ConnectSurfacePalette(theme: .light).uiColor(.homeText) : contentColor
    }

    private var subduedContentColor: UIColor {
        switch tone {
        case .light: UIColor(white: 1.0, alpha: 0.8)
        case .dark: UIColor(white: 0.0, alpha: 0.8)
        }
    }

    private func applyTone() {
        wordmarkImageView.image = UIImage(named: "WarrenWordmark")?
            .withTintColor(wordmarkColor, renderingMode: .alwaysOriginal)
        let glow = ConnectSurfacePalette(theme: .light).uiColor(.homeLogoGlow).withAlphaComponent(1)
        wordmarkGlowView.image = UIImage(named: "WarrenWordmark")?
            .withTintColor(glow, renderingMode: .alwaysOriginal)
        wordmarkGlowView.layer.shadowColor = glow.cgColor
        wordmarkImageView.layer.shadowColor = glow.cgColor
        if let homeTheme {
            toolbarBackground.apply(theme: homeTheme)
            let palette = ConnectSurfacePalette(theme: homeTheme)
            forumBadgeLabel.backgroundColor = palette.uiColor(.homeBadge)
            forumBadgeLabel.textColor = palette.uiColor(.homePillPendingText)
            forumBadgeLabel.layer.borderColor = palette.uiColor(.homeBadgeRing).cgColor
        } else {
            forumBadgeLabel.backgroundColor = UIColor.Warren.yellow
            forumBadgeLabel.textColor = UIColor.Warren.navy
            forumBadgeLabel.layer.borderColor = nil
        }
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
        // The forum glyph rides the same two backdrops as the rest, so it is
        // re-tinted here rather than fixed at init.
        applyForumSlot()
        // The chip has to be re-tinted here with everything else. It rides two
        // very different backdrops (the charcoal bar, and the bright scenery
        // sky on the connect screen), and an edge left in one tone's colour
        // dissolves into the other.
        productChipLabel.backgroundColor = UIColor.Warren.yellow
        productChipLabel.textColor = UIColor.Warren.navy
        productChipLabel.layer.borderColor = contentColor.withAlphaComponent(0.35).cgColor
    }

    /// Swaps the classic bar for the home's floating wordmark and toolbar.
    private func applyLayout(home: Bool, wasHome: Bool) {
        guard home != wasHome else { return }
        if home {
            NSLayoutConstraint.deactivate(classicConstraints)
            NSLayoutConstraint.activate(homeConstraints)
        } else {
            NSLayoutConstraint.deactivate(homeConstraints)
            NSLayoutConstraint.activate(classicConstraints)
        }
        let buttonSize = home ? HomeMetrics.Toolbar.button : UIMetrics.Button.barButtonSize
        for button in [forumButton, accountButton, settingsButton] {
            button.constraints
                .filter { $0.firstItem === button && $0.firstAttribute == .height && $0.secondItem == nil }
                .forEach { $0.constant = buttonSize }
            button.focusEffect = home ? Self.homeToolbarFocusRing() : nil
        }
        buttonContainer.spacing = home ? 0 : 12
        toolbarBackground.isHidden = !home
        wordmarkGlowView.isHidden = !home
        wordmarkImageView.layer.shadowOpacity = home ? 0.95 : 0
        wordmarkImageView.layer.shadowRadius = home ? 1.5 : 0
        wordmarkImageView.layer.shadowOffset = .zero
        forumBadgeLabel.layer.borderWidth = home ? HomeMetrics.Toolbar.badgeRing : 0
        deviceInfoHolder.isHidden = home
    }

    /// The system rings a focused button outside its bounds, which on the home
    /// spills over the pill's edge; there the ring is the hit area's rounded
    /// square, drawn inside it (desktop `HomeHeader`).
    private static func homeToolbarFocusRing() -> UIFocusHaloEffect {
        let toolbar = HomeMetrics.Toolbar.self
        let ring = UIFocusHaloEffect(
            roundedRect: CGRect(x: 0, y: 0, width: toolbar.button, height: toolbar.button),
            cornerRadius: toolbar.radius - toolbar.paddingHorizontal,
            curve: .continuous
        )
        ring.position = .inside
        return ring
    }

    /// The glyph, the badge and the spoken label of the forum slot, all from
    /// the same two facts, so what is drawn and what is read out can never
    /// disagree.
    private func applyForumSlot() {
        forumButton.isHidden = forumSlot == .none
        let symbol = forumSlot == .community ? "lifepreserver" : "bell"
        forumButton.setImage(
            UIImage(systemName: symbol)?
                .withTintColor(contentColor, renderingMode: .alwaysOriginal),
            for: .normal
        )
        let showsCount = forumSlot == .activity && forumUnread > 0
        forumBadgeLabel.isHidden = !showsCount
        forumBadgeLabel.text = showsCount ? WarrenForumActivity.label(unread: forumUnread) : nil
        forumButton.accessibilityLabel = forumSlotAccessibilityLabel(showsCount: showsCount)
    }

    private func forumSlotAccessibilityLabel(showsCount: Bool) -> String {
        guard forumSlot == .activity else {
            return NSLocalizedString(
                "Community forum",
                tableName: "Settings",
                comment: ""
            )
        }
        guard showsCount else {
            return NSLocalizedString(
                "Forum activity",
                tableName: "Settings",
                comment: ""
            )
        }
        return String(
            format: NSLocalizedString(
                "Forum activity, %@ waiting",
                tableName: "Settings",
                comment: ""
            ),
            WarrenForumActivity.label(unread: forumUnread)
        )
    }

    private func applyProductBadge() {
        productChipLabel.text = productBadge
        let showsChip = productBadge != nil && !isProductChipSuppressed
        productChipLabel.isHidden = !showsChip
        // The chip is the only place in the app that names this build's
        // network, so on a marked build it is also the way in to what that
        // network costs the user in speed. Prod shows no chip and nothing to
        // open.
        productChipLabel.isUserInteractionEnabled = showsChip
        productChipLabel.isAccessibilityElement = showsChip
        productChipLabel.accessibilityTraits = .button
        productChipLabel.accessibilityLabel = productBadge.map { badge in
            String(
                format: NSLocalizedString(
                    "%@ build, about this network",
                    tableName: "Settings",
                    comment: ""
                ),
                badge
            )
        }
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

        forumButton.addConstrainedSubviews([forumBadgeLabel]) {
            forumBadgeLabel.topAnchor.constraint(equalTo: forumButton.topAnchor, constant: -2)
            forumBadgeLabel.leadingAnchor.constraint(
                equalTo: forumButton.centerXAnchor,
                constant: 2
            )
            forumBadgeLabel.heightAnchor.constraint(greaterThanOrEqualToConstant: 14)
            forumBadgeLabel.widthAnchor.constraint(
                greaterThanOrEqualTo: forumBadgeLabel.heightAnchor
            )
        }

        [deviceNameLabel, timeLeftLabel].forEach { deviceInfoHolder.addArrangedSubview($0) }

        [wordmarkGlowView, toolbarBackground].forEach { $0.configureForAutoLayout() }
        addSubview(wordmarkGlowView)
        addSubview(toolbarBackground)
        addConstrainedSubviews([wordmarkImageView, productChipLabel, buttonContainer, deviceInfoHolder]) {
            wordmarkImageView.widthAnchor.constraint(
                equalTo: wordmarkImageView.heightAnchor,
                multiplier: wordmarkAspectRatio
            )

            // The chip rides the letter band of the lockup, the same optical
            // line the buttons take, so it reads as part of the wordmark
            // instead of floating over the ears.
            productChipLabel.leadingAnchor.constraint(
                equalTo: wordmarkImageView.trailingAnchor,
                constant: 8
            )
            productChipLabel.centerYAnchor.constraint(
                equalTo: wordmarkImageView.centerYAnchor,
                constant: 5
            )

            deviceInfoHolder.leadingAnchor.constraint(equalTo: layoutMarginsGuide.leadingAnchor)
            deviceInfoHolder.trailingAnchor.constraint(equalTo: layoutMarginsGuide.trailingAnchor)

            // The glow is the wordmark again, in paper, under it: its shadow
            // is the wide halo, the wordmark's own the tight one.
            wordmarkGlowView.leadingAnchor.constraint(equalTo: wordmarkImageView.leadingAnchor)
            wordmarkGlowView.trailingAnchor.constraint(equalTo: wordmarkImageView.trailingAnchor)
            wordmarkGlowView.topAnchor.constraint(equalTo: wordmarkImageView.topAnchor)
            wordmarkGlowView.bottomAnchor.constraint(equalTo: wordmarkImageView.bottomAnchor)

            toolbarBackground.leadingAnchor.constraint(
                equalTo: buttonContainer.leadingAnchor,
                constant: -HomeMetrics.Toolbar.paddingHorizontal
            )
            toolbarBackground.trailingAnchor.constraint(
                equalTo: buttonContainer.trailingAnchor,
                constant: HomeMetrics.Toolbar.paddingHorizontal
            )
            toolbarBackground.centerYAnchor.constraint(equalTo: buttonContainer.centerYAnchor)
            toolbarBackground.heightAnchor.constraint(equalToConstant: HomeMetrics.Toolbar.height)
        }

        classicConstraints = [
            wordmarkImageView.leadingAnchor.constraint(equalTo: layoutMarginsGuide.leadingAnchor),
            wordmarkImageView.topAnchor.constraint(
                equalTo: layoutMarginsGuide.topAnchor,
                constant: 8
            ),
            wordmarkImageView.heightAnchor.constraint(equalToConstant: UIMetrics.headerBarLockupHeight),

            // The lockup's letters sit in the bottom band of its box (the W's
            // ears tower above), so the buttons center on the letter band, the
            // optical alignment the eye expects, not on the box middle.
            buttonContainer.centerYAnchor.constraint(
                equalTo: wordmarkImageView.centerYAnchor,
                constant: 5
            ),
            buttonContainer.trailingAnchor.constraint(
                equalTo: layoutMarginsGuide.trailingAnchor,
                constant: buttonContainerTrailingAdjustment
            ),

            deviceInfoHolder.topAnchor.constraint(
                equalToSystemSpacingBelow: wordmarkImageView.bottomAnchor,
                multiplier: 1
            ),
            layoutMarginsGuide.bottomAnchor.constraint(
                equalToSystemSpacingBelow: deviceInfoHolder.bottomAnchor,
                multiplier: 1
            ),
        ]

        // The home floats the inked wordmark and a toolbar pill over the art
        // (desktop `HomeHeader`): both inset from the screen edges, the
        // toolbar under the status bar, and the header ending just below it,
        // where the notification banners hang.
        let toolbar = HomeMetrics.Toolbar.self
        let inset = HomeMetrics.Hero.headerInset
        homeConstraints = [
            wordmarkImageView.leadingAnchor.constraint(
                equalTo: safeAreaLayoutGuide.leadingAnchor,
                constant: inset + 4
            ),
            wordmarkImageView.heightAnchor.constraint(equalToConstant: HomeMetrics.Hero.logoHeight),
            wordmarkImageView.centerYAnchor.constraint(equalTo: toolbarBackground.centerYAnchor),
            buttonContainer.topAnchor.constraint(
                equalTo: layoutMarginsGuide.topAnchor,
                constant: inset + (toolbar.height - toolbar.button) / 2
            ),
            buttonContainer.trailingAnchor.constraint(
                equalTo: safeAreaLayoutGuide.trailingAnchor,
                constant: -(inset + toolbar.paddingHorizontal)
            ),
            bottomAnchor.constraint(equalTo: toolbarBackground.bottomAnchor, constant: 8),
        ]
        NSLayoutConstraint.activate(classicConstraints)

        productChipLabel.addGestureRecognizer(
            UITapGestureRecognizer(target: self, action: #selector(handleProductChipTap))
        )

        applyProductBadge()
        applyTone()
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    /// What a tap on the product chip opens. Set by the container, which owns
    /// the presentation.
    var onProductChipTap: (() -> Void)?

    @objc private func handleProductChipTap() {
        onProductChipTap?()
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

/// The home toolbar's pill: a blurred glass of the page colour with a hairline
/// inset, a dark glass in the dark theme and a paper one in the light theme
/// (desktop `HomeHeader`'s `StyledToolbar`). The shadow lives on this view and
/// the clipped glass inside it, since a clipping layer cannot cast one.
final class HomeToolbarBackgroundView: UIView {
    private let blurView = UIVisualEffectView(effect: nil)
    private let tintView = UIView()

    override init(frame: CGRect) {
        super.init(frame: frame)
        isHidden = true
        isUserInteractionEnabled = false
        let radius = HomeMetrics.Toolbar.radius
        blurView.layer.cornerRadius = radius
        blurView.layer.cornerCurve = .continuous
        blurView.clipsToBounds = true
        tintView.layer.cornerRadius = radius
        tintView.layer.cornerCurve = .continuous
        tintView.layer.borderWidth = 1
        layer.shadowOffset = CGSize(width: 0, height: 4)
        layer.shadowRadius = 5
        layer.shadowOpacity = 1
        [blurView, tintView].forEach { addSubview($0) }
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    func apply(theme: WarrenTheme) {
        let palette = ConnectSurfacePalette(theme: theme)
        blurView.effect = UIBlurEffect(
            style: theme == .dark ? .systemUltraThinMaterialDark : .systemUltraThinMaterialLight
        )
        tintView.backgroundColor = palette.uiColor(.homeToolbar)
        tintView.layer.borderColor = palette.uiColor(.homeToolbarLine).cgColor
        layer.shadowColor = palette.uiColor(.homeToolbarShadow).cgColor
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        blurView.frame = bounds
        tintView.frame = bounds
        let shadowShape = UIBezierPath(
            roundedRect: bounds.insetBy(dx: 3, dy: 3),
            cornerRadius: HomeMetrics.Toolbar.radius
        )
        layer.shadowPath = shadowShape.cgPath
    }
}
