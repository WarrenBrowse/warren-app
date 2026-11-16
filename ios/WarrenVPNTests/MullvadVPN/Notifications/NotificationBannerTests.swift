//
//  NotificationBannerTests.swift
//  WarrenVPNTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import UIKit
import WarrenTypes
import XCTest

@testable import WarrenVPN

/// The home's in-app banner, mockup 8d (desktop `NotificationBanner.tsx`): a compact card of the
/// toolbar's glass hanging under the toolbar on its right edge, a glyph in front of the words,
/// and either a labelled button or a bare cross at the end.
@MainActor
final class NotificationBannerTests: XCTestCase {
    private func descriptor(button: InAppNotificationAction?) -> InAppNotificationDescriptor {
        InAppNotificationDescriptor(
            identifier: .newAppVersionInAppNotification,
            style: .warning,
            title: "Update available",
            body: NSAttributedString(string: "Version 2026.5"),
            button: button
        )
    }

    func testAnUpdateLeadsWithTheAmberDownload() {
        let lead = NotificationBannerView.lead(icon: .update, style: .warning)
        XCTAssertEqual(lead.systemName, "square.and.arrow.down")
        XCTAssertEqual(lead.token, .homeNoticeUpdate)
    }

    func testAnAnnouncementLeadsWithAQuietMegaphoneWhateverItsLevel() {
        for style in [NotificationBannerStyle.success, .warning, .error] {
            let lead = NotificationBannerView.lead(icon: .announcement, style: style)
            XCTAssertEqual(lead.systemName, "megaphone", "\(style)")
            XCTAssertEqual(lead.token, .homeNoticeAnnouncement, "\(style)")
        }
    }

    func testWithoutAGlyphOfItsOwnTheLevelPicksOne() {
        let success = NotificationBannerView.lead(icon: nil, style: .success)
        let warning = NotificationBannerView.lead(icon: nil, style: .warning)
        let error = NotificationBannerView.lead(icon: nil, style: .error)
        XCTAssertEqual(success.systemName, "info.circle")
        XCTAssertEqual(success.token, .homeNoticeTextSecondary)
        XCTAssertEqual(warning.systemName, "exclamationmark.circle")
        XCTAssertEqual(warning.token, .homeBadge)
        XCTAssertEqual(error.systemName, "exclamationmark.circle")
        XCTAssertEqual(error.token, .homeDisconnectText)
    }

    /// 10 points under the toolbar's bottom edge (the container's top on the home), flush with the
    /// toolbar's right edge, and never wider than the toolbar's reach.
    func testTheCardHangsUnderTheToolbarOnItsRightEdge() {
        let controller = NotificationController()
        controller.loadViewIfNeeded()
        controller.view.frame = CGRect(x: 0, y: 0, width: 393, height: 700)
        let long = String(repeating: "A body long enough to wrap over several lines. ", count: 4)
        var notification = descriptor(button: InAppNotificationAction(title: "Install", handler: {}))
        notification.body = NSAttributedString(string: long)
        controller.setNotifications([notification], animated: false)
        controller.view.layoutIfNeeded()

        let card = controller.bannerView.cardView.convert(controller.bannerView.cardView.bounds, to: controller.view)
        XCTAssertEqual(card.minY, HomeMetrics.Notice.marginTop, accuracy: 0.5)
        XCTAssertEqual(card.maxX, 393 - HomeMetrics.Hero.headerInset, accuracy: 0.5)
        XCTAssertEqual(card.width, HomeMetrics.Notice.maxWidth, accuracy: 0.5)
    }

    /// A short banner is as wide as its words need, still on the toolbar's right edge.
    func testAShortCardHugsItsWords() {
        let controller = NotificationController()
        controller.loadViewIfNeeded()
        controller.view.frame = CGRect(x: 0, y: 0, width: 393, height: 700)
        controller.setNotifications([descriptor(button: nil)], animated: false)
        controller.view.layoutIfNeeded()

        let card = controller.bannerView.cardView.convert(controller.bannerView.cardView.bounds, to: controller.view)
        XCTAssertLessThan(card.width, HomeMetrics.Notice.maxWidth)
        XCTAssertEqual(card.maxX, 393 - HomeMetrics.Hero.headerInset, accuracy: 0.5)
    }

    /// A titled action is the light "Install" pill and runs its handler; an untitled one is the
    /// bare cross.
    func testATitledActionIsALabelledButton() {
        var installed = false
        let banner = NotificationBannerView()
        banner.action = InAppNotificationAction(title: "Install", handler: { installed = true })

        XCTAssertEqual(banner.actionButton.configuration?.title, "Install")
        banner.actionButton.sendActions(for: .touchUpInside)
        XCTAssertTrue(installed)

        banner.action = InAppNotificationAction(image: UIImage.Buttons.bannerClose, handler: {})
        XCTAssertNil(banner.actionButton.configuration?.title)
        XCTAssertNotNil(banner.actionButton.configuration?.image)
    }

    /// A card that ends on its button sits tighter around it (8 8 8 12) than one that ends on words
    /// or the cross (9 10 9 12), and the button is the mockup's 30 point pill.
    func testACardEndingOnAButtonTightensItsPadding() {
        let banner = NotificationBannerView(frame: CGRect(x: 0, y: 0, width: 393, height: 120))
        banner.title = "Update available"
        banner.action = InAppNotificationAction(title: "Install", handler: {})
        XCTAssertEqual(banner.contentPadding, NSDirectionalEdgeInsets(top: 8, leading: 12, bottom: 8, trailing: 8))
        banner.layoutIfNeeded()
        XCTAssertEqual(banner.actionButton.bounds.height, HomeMetrics.Notice.buttonHeight, accuracy: 0.01)

        banner.action = InAppNotificationAction(image: UIImage.Buttons.bannerClose, handler: {})
        XCTAssertEqual(banner.contentPadding, NSDirectionalEdgeInsets(top: 9, leading: 12, bottom: 9, trailing: 10))
    }

    /// Words a provider emphasises in white would vanish on the paper glass: the body is redrawn
    /// in the theme's secondary text, its emphasis in the theme's main text.
    func testTheBodyTakesTheThemeColours() {
        let body = NSMutableAttributedString(string: "Install the latest version.\n")
        body.append(
            NSAttributedString(string: "Tap here", attributes: [.foregroundColor: UIColor.white]))
        let palette = ConnectSurfacePalette(theme: .light)

        let styled = NotificationBannerView.styledBody(body, palette: palette)

        let plain = styled.attribute(.foregroundColor, at: 0, effectiveRange: nil) as? UIColor
        let emphasis = styled.attribute(.foregroundColor, at: styled.length - 1, effectiveRange: nil) as? UIColor
        XCTAssertEqual(plain, palette.uiColor(.homeNoticeTextSecondary))
        XCTAssertEqual(emphasis, palette.uiColor(.homeText))
    }

    // MARK: - The update banner

    /// "Update available", "Version X" and an Install button that takes the update path.
    func testTheUpdateBannerNamesTheVersionAndOffersInstall() throws {
        var installed = 0
        let descriptor = NewAppVersionInAppNotificationProvider.descriptor(
            version: "2026.5",
            includeAllNetworksIsEnabled: false,
            install: { installed += 1 }
        )

        XCTAssertEqual(descriptor.title, "Update available")
        XCTAssertEqual(descriptor.body.string, "Version 2026.5")
        XCTAssertEqual(descriptor.icon, .update)
        XCTAssertEqual(descriptor.button?.title, "Install")
        descriptor.button?.handler?()
        descriptor.tapAction?.handler?()
        XCTAssertEqual(installed, 2)
    }

    /// With "Force all apps" on, updating would cut the network: the banner keeps saying so.
    func testTheUpdateBannerKeepsTheForceAllAppsWarning() {
        let descriptor = NewAppVersionInAppNotificationProvider.descriptor(
            version: "2026.5",
            includeAllNetworksIsEnabled: true,
            install: {}
        )

        XCTAssertTrue(descriptor.body.string.contains("Force all apps"), descriptor.body.string)
        XCTAssertEqual(descriptor.button?.title, "Install")
    }
}
