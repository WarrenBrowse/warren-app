//
//  NotificationBannerView.swift
//  MullvadVPN
//
//  Created by pronebird on 01/06/2021.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import UIKit
import WarrenTypes

/// The in-app banner, mockup 8d (desktop `NotificationBanner.tsx`): a compact card of the home
/// toolbar's own glass that hangs under the toolbar on its right edge, as wide as its words need
/// up to the toolbar's reach. A glyph leads in place of a coloured dot, the title and the body
/// follow, and the card ends with a labelled button or a bare glyph.
final class NotificationBannerView: UIView {
    enum Metrics {
        static let gapBelowToolbar: CGFloat = 12
        static let maxWidth: CGFloat = 272
        static let radius: CGFloat = 12
        static let padding = NSDirectionalEdgeInsets(top: 10, leading: 12, bottom: 10, trailing: 12)
        static let gap: CGFloat = 10
        static let icon: CGFloat = 16
        static let titleSize: CGFloat = 13
        static let bodySize: CGFloat = 11
        static let lineGap: CGFloat = 1
        static let buttonRadius: CGFloat = 8
        static let buttonTextSize: CGFloat = 12
    }

    /// The card itself; the rest of the view is the transparent row it sits in.
    let cardView = UIView()
    private let glassView = HomeToolbarBackgroundView(radius: Metrics.radius)

    private let leadIconView: UIImageView = {
        let imageView = UIImageView()
        imageView.contentMode = .center
        imageView.setContentHuggingPriority(.required, for: .horizontal)
        imageView.setContentCompressionResistancePriority(.required, for: .horizontal)
        return imageView
    }()

    private let titleLabel: UILabel = {
        let textLabel = UILabel()
        textLabel.font = .instrumentSans(size: Metrics.titleSize, weight: .semibold, relativeTo: .footnote)
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
        stackView.spacing = Metrics.lineGap
        return stackView
    }()

    private lazy var contentStackView: UIStackView = {
        let stackView = UIStackView(arrangedSubviews: [leadIconView, textStackView, actionButton])
        stackView.axis = .horizontal
        stackView.alignment = .center
        stackView.spacing = Metrics.gap
        stackView.isLayoutMarginsRelativeArrangement = true
        stackView.directionalLayoutMargins = Metrics.padding
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

    override func layoutSubviews() {
        updateTextWidth()
        super.layoutSubviews()
    }

    private func updateTextWidth() {
        let inset = HomeMetrics.Hero.headerInset
        let cardWidth = bounds.width > 0 ? min(Metrics.maxWidth, bounds.width - 2 * inset) : Metrics.maxWidth
        var room = cardWidth - Metrics.padding.leading - Metrics.padding.trailing - Metrics.icon - Metrics.gap
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
            cardView.topAnchor.constraint(equalTo: topAnchor, constant: Metrics.gapBelowToolbar)
            cardView.bottomAnchor.constraint(equalTo: bottomAnchor)
            cardView.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -inset)
            cardView.leadingAnchor.constraint(greaterThanOrEqualTo: leadingAnchor, constant: inset)
            cardView.widthAnchor.constraint(lessThanOrEqualToConstant: Metrics.maxWidth)
        }
    }

    private func applyContent() {
        let palette = ConnectSurfacePalette(theme: theme)
        glassView.apply(theme: theme)
        // The banner's own drop, deeper than the toolbar's (desktop `0 8px 24px -8px`).
        glassView.layer.shadowColor = UIColor.black.cgColor
        glassView.layer.shadowOpacity = 0.45
        glassView.layer.shadowRadius = 8
        glassView.layer.shadowOffset = CGSize(width: 0, height: 8)

        let lead = Self.lead(icon: icon, style: style)
        leadIconView.image = UIImage(
            systemName: lead.systemName,
            withConfiguration: UIImage.SymbolConfiguration(pointSize: 14, weight: .medium)
        )
        leadIconView.tintColor = palette.uiColor(lead.token)

        titleLabel.textColor = palette.uiColor(.homeText)
        bodyLabel.attributedText = body.map { Self.styledBody($0, palette: palette) }
        bodyLabel.isHidden = (body?.length ?? 0) == 0

        actionButton.configuration = Self.buttonConfiguration(for: action, palette: palette)
        actionButton.isHidden = actionButton.configuration == nil
        setNeedsLayout()
    }

    /// A titled action is the mockup's light "Install"; one with only an image is that image as a
    /// bare 16 point glyph in the secondary text.
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
            configuration.contentInsets = NSDirectionalEdgeInsets(top: 6, leading: 10, bottom: 6, trailing: 10)
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
                .resized(to: CGSize(width: Metrics.icon, height: Metrics.icon))
                .withRenderingMode(.alwaysTemplate)
            configuration.baseForegroundColor = palette.uiColor(.homeTextSecondary)
            configuration.contentInsets = NSDirectionalEdgeInsets(top: 4, leading: 4, bottom: 4, trailing: 4)
            return configuration
        }
        return nil
    }

    /// The glyph a banner leads with (desktop `NotificationLeadIcon`): the amber download for an
    /// update, a quiet megaphone for an announcement, and otherwise one its level picks.
    static func lead(
        icon: NotificationBannerIcon?,
        style: NotificationBannerStyle
    ) -> (systemName: String, token: ConnectSurfaceToken) {
        switch (icon, style) {
        case (.update, _): ("square.and.arrow.down", .homeBadge)
        case (.announcement, _): ("megaphone", .homeTextSecondary)
        case (nil, .success): ("info.circle", .homeTextSecondary)
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
                    .foregroundColor: palette.uiColor(emphasised ? .homeText : .homeTextSecondary),
                    .font: emphasised ? emphasis : regular,
                ],
                range: range
            )
        }
        return styled
    }
}
