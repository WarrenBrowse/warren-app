//
//  NotificationBannerView.swift
//  MullvadVPN
//
//  Created by pronebird on 01/06/2021.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import UIKit
import WarrenTypes

/// The in-app banner of the 380x620 mockup (desktop `NotificationBanner.tsx`, `notice` tokens): a
/// compact glass card that hangs under the toolbar on its right edge, as wide as its words need up
/// to its cap. A glyph leads in place of a coloured dot, the title and the body follow, and the
/// card ends with a labelled button or the bare cross.
final class NotificationBannerView: UIView {
    private typealias Metrics = HomeMetrics.Notice

    /// The card itself; the rest of the view is the transparent row it sits in.
    let cardView = UIView()
    private let glassView = HomeToolbarBackgroundView(radius: Metrics.radius, surface: .notice)

    private let leadIconView: UIImageView = {
        let imageView = UIImageView()
        imageView.contentMode = .center
        imageView.setContentHuggingPriority(.required, for: .horizontal)
        imageView.setContentCompressionResistancePriority(.required, for: .horizontal)
        return imageView
    }()

    private let titleLabel: UILabel = {
        let textLabel = UILabel()
        textLabel.font = .instrumentSans(size: HomeMetrics.Notice.titleSize, weight: .semibold, relativeTo: .footnote)
        textLabel.adjustsFontForContentSizeCategory = true
        textLabel.numberOfLines = 0
        textLabel.lineBreakMode = .byWordWrapping
        textLabel.lineBreakStrategy = []
        return textLabel
    }()

    private let bodyLabel: UILabel = {
        let textLabel = UILabel()
        textLabel.adjustsFontForContentSizeCategory = true
        textLabel.numberOfLines = 0
        textLabel.lineBreakMode = .byWordWrapping
        textLabel.lineBreakStrategy = []
        return textLabel
    }()

    private lazy var textStackView: UIStackView = {
        let stackView = UIStackView(arrangedSubviews: [titleLabel, bodyLabel])
        stackView.axis = .vertical
        stackView.alignment = .leading
        stackView.spacing = HomeMetrics.Notice.bodyGap
        return stackView
    }()

    private lazy var contentStackView: UIStackView = {
        let stackView = UIStackView(arrangedSubviews: [leadIconView, textStackView, actionButton])
        stackView.axis = .horizontal
        stackView.alignment = .center
        stackView.spacing = HomeMetrics.Notice.gap
        stackView.isLayoutMarginsRelativeArrangement = true
        return stackView
    }()

    let actionButton: IncreasedHitButton = {
        let button = IncreasedHitButton(type: .custom)
        button.setContentHuggingPriority(.required, for: .horizontal)
        button.setContentCompressionResistancePriority(.required, for: .horizontal)
        return button
    }()

    /// The palette the card paints in: the home's theme.
    var theme: WarrenTheme = .dark {
        didSet {
            guard theme != oldValue else { return }
            applyContent()
        }
    }

    var title: String? {
        didSet {
            titleLabel.text = title
            setNeedsLayout()
        }
    }

    var body: NSAttributedString? {
        didSet { applyContent() }
    }

    var style: NotificationBannerStyle = .error {
        didSet { applyContent() }
    }

    var icon: NotificationBannerIcon? {
        didSet { applyContent() }
    }

    var action: InAppNotificationAction? {
        didSet { applyContent() }
    }

    var tapAction: InAppNotificationAction?

    override init(frame: CGRect) {
        super.init(frame: frame)
        addSubviews()
        addTapHandler()
        actionButton.addTarget(self, action: #selector(handleActionTap), for: .touchUpInside)
        applyContent()
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    /// The words' column is as wide as its longer line needs, up to the room the capped card
    /// leaves it, so a short banner hugs its words and a long one fills the card and wraps.
    private lazy var textWidth = textStackView.widthAnchor.constraint(equalToConstant: 0)
    private lazy var buttonHeight = actionButton.heightAnchor.constraint(equalToConstant: Metrics.buttonHeight)

    override func layoutSubviews() {
        updateTextWidth()
        super.layoutSubviews()
    }

    /// The card's padding: tighter around a closing button, as the mockup draws it.
    var contentPadding: NSDirectionalEdgeInsets {
        let endsOnButton = action?.title != nil
        let vertical = endsOnButton ? Metrics.buttonInset : Metrics.paddingVertical
        return NSDirectionalEdgeInsets(
            top: vertical,
            leading: Metrics.paddingStart,
            bottom: vertical,
            trailing: endsOnButton ? Metrics.buttonInset : Metrics.paddingEnd
        )
    }

    private func updateTextWidth() {
        let inset = HomeMetrics.Hero.headerInset
        let cardWidth = bounds.width > 0 ? min(Metrics.maxWidth, bounds.width - 2 * inset) : Metrics.maxWidth
        let padding = contentPadding
        var room = cardWidth - padding.leading - padding.trailing - Metrics.icon - Metrics.gap
        if !actionButton.isHidden {
            room -= actionButton.intrinsicContentSize.width + Metrics.gap
        }
        let unbounded = CGSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)
        let needed =
            [titleLabel, bodyLabel]
            .filter { !$0.isHidden }
            .map { ceil($0.sizeThatFits(unbounded).width) }
            .max() ?? 0
        let width = max(0, min(room, needed))
        guard textWidth.constant != width || !textWidth.isActive else { return }
        textWidth.constant = width
        textWidth.isActive = true
        titleLabel.preferredMaxLayoutWidth = width
        bodyLabel.preferredMaxLayoutWidth = width
    }

    // Only the card is tappable; taps beside it fall through to the screen underneath.
    override func point(inside point: CGPoint, with event: UIEvent?) -> Bool {
        cardView.frame.contains(point)
    }

    private func addTapHandler() {
        let tapGesture = UITapGestureRecognizer(target: self, action: #selector(handleTap))
        cardView.addGestureRecognizer(tapGesture)
    }

    @objc
    private func handleTap() {
        tapAction?.handler?()
    }

    @objc private func handleActionTap() {
        action?.handler?()
    }

    private func addSubviews() {
        glassView.isHidden = false
        cardView.addConstrainedSubviews([glassView, contentStackView]) {
            glassView.pinEdgesToSuperview()
            contentStackView.pinEdgesToSuperview()
            leadIconView.widthAnchor.constraint(equalToConstant: Metrics.icon)
            leadIconView.heightAnchor.constraint(equalToConstant: Metrics.icon)
        }

        // On the toolbar's right edge, as wide as the words need up to the toolbar's reach.
        let inset = HomeMetrics.Hero.headerInset
        addConstrainedSubviews([cardView]) {
            cardView.topAnchor.constraint(equalTo: topAnchor, constant: Metrics.marginTop)
            cardView.bottomAnchor.constraint(equalTo: bottomAnchor)
            cardView.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -inset)
            cardView.leadingAnchor.constraint(greaterThanOrEqualTo: leadingAnchor, constant: inset)
            cardView.widthAnchor.constraint(lessThanOrEqualToConstant: Metrics.maxWidth)
        }
    }

    private func applyContent() {
        let palette = ConnectSurfacePalette(theme: theme)
        glassView.apply(theme: theme)
        contentStackView.directionalLayoutMargins = contentPadding

        let lead = Self.lead(icon: icon, style: style)
        leadIconView.image = UIImage(
            systemName: lead.systemName,
            withConfiguration: UIImage.SymbolConfiguration(pointSize: 17, weight: .regular)
        )
        leadIconView.tintColor = palette.uiColor(lead.token)

        titleLabel.textColor = palette.uiColor(.homeText)
        bodyLabel.attributedText = body.map { Self.styledBody($0, palette: palette) }
        bodyLabel.isHidden = (body?.length ?? 0) == 0

        actionButton.configuration = Self.buttonConfiguration(for: action, palette: palette)
        actionButton.isHidden = actionButton.configuration == nil
        buttonHeight.isActive = action?.title != nil
        setNeedsLayout()
    }

    /// A titled action is the mockup's light "Install", 30 high; one with only an image is that
    /// image as the bare 18 point cross in the notice's secondary text.
    private static func buttonConfiguration(
        for action: InAppNotificationAction?,
        palette: ConnectSurfacePalette
    ) -> UIButton.Configuration? {
        if let title = action?.title {
            var configuration = UIButton.Configuration.filled()
            configuration.title = title
            configuration.baseBackgroundColor = palette.uiColor(.homeText)
            configuration.baseForegroundColor = palette.uiColor(.homePage)
            configuration.cornerStyle = .fixed
            configuration.background.cornerRadius = Metrics.buttonRadius
            configuration.contentInsets = NSDirectionalEdgeInsets(
                top: 0, leading: Metrics.buttonPaddingHorizontal, bottom: 0, trailing: Metrics.buttonPaddingHorizontal)
            configuration.titleTextAttributesTransformer = UIConfigurationTextAttributesTransformer { attributes in
                var attributes = attributes
                attributes.font = UIFont.instrumentSans(
                    size: Metrics.buttonTextSize, weight: .semibold, relativeTo: .footnote)
                return attributes
            }
            return configuration
        }
        if let image = action?.image {
            var configuration = UIButton.Configuration.plain()
            configuration.image =
                image
                .resized(to: CGSize(width: Metrics.close, height: Metrics.close))
                .withRenderingMode(.alwaysTemplate)
            configuration.baseForegroundColor = palette.uiColor(.homeNoticeTextSecondary)
            configuration.contentInsets = NSDirectionalEdgeInsets(top: 0, leading: 0, bottom: 0, trailing: 0)
            return configuration
        }
        return nil
    }

    /// The glyph a banner leads with (desktop `noticeIconColor`): the amber download for an
    /// update, the blue megaphone for an announcement, and otherwise one its level picks.
    static func lead(
        icon: NotificationBannerIcon?,
        style: NotificationBannerStyle
    ) -> (systemName: String, token: ConnectSurfaceToken) {
        switch (icon, style) {
        case (.update, _): ("square.and.arrow.down", .homeNoticeUpdate)
        case (.announcement, _): ("megaphone", .homeNoticeAnnouncement)
        case (nil, .success): ("info.circle", .homeNoticeTextSecondary)
        case (nil, .warning): ("exclamationmark.circle", .homeBadge)
        case (nil, .error): ("exclamationmark.circle", .homeDisconnectText)
        }
    }

    /// The body in the theme's secondary text. The words a provider sets apart by colour (a "Tap
    /// here", a bold markdown run) take the theme's main text instead, so they read on the dark
    /// glass and on the paper one.
    static func styledBody(_ body: NSAttributedString, palette: ConnectSurfacePalette) -> NSAttributedString {
        let styled = NSMutableAttributedString(attributedString: body)
        let regular = UIFont.instrumentSans(size: Metrics.bodySize, relativeTo: .caption2)
        let emphasis = UIFont.instrumentSans(size: Metrics.bodySize, weight: .semibold, relativeTo: .caption2)
        body.enumerateAttribute(
            .foregroundColor,
            in: NSRange(location: 0, length: body.length)
        ) { color, range, _ in
            let emphasised = color != nil
            styled.addAttributes(
                [
                    .foregroundColor: palette.uiColor(emphasised ? .homeText : .homeNoticeTextSecondary),
                    .font: emphasised ? emphasis : regular,
                ],
                range: range
            )
        }
        return styled
    }
}
