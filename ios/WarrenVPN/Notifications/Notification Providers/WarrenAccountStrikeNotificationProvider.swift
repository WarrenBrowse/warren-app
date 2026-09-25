//
//  WarrenAccountStrikeNotificationProvider.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import Foundation
import WarrenRustRuntime
import WarrenSettings
import WarrenTypes
import UIKit.UIImage

/// Where the reader's strike dismissals are kept. Behind a protocol so the
/// banner can be exercised without UserDefaults.
protocol WarrenStrikeDismissing: AnyObject {
    /// Digests of the strikes whose banner the reader has put away.
    var warrenDismissedStrikes: [String] { get set }
}

extension AppPreferences: WarrenStrikeDismissing {}

/// A forwarded port was closed after an abuse report and counted against the
/// account (warren-core doc 105, desktop `WarrenAccountStrikeNotificationProvider`):
/// the newest live warning with its case reference, and a tap on it opens the
/// page on contesting it. It can be put away, keyed on the strike, because it
/// would otherwise hold the banner for the whole window; the warning stays
/// listed in the port-forwarding screen, and the next strike raises the
/// banner again.
final class WarrenAccountStrikeNotificationProvider: NotificationProvider,
    InAppNotificationProvider, @unchecked Sendable
{
    /// The strike the banner shows, `nil` when there is none to show.
    static func shouldDisplay(
        standing: WarrenAccountStanding?,
        dismissed: [String]
    ) -> WarrenStrikeNotice? {
        guard let latest = standing?.latestStrike,
            !dismissed.contains(latest.strike.dismissalKey)
        else {
            return nil
        }
        return latest
    }

    private let source: () -> WarrenAccountStanding?
    private let dismissStore: WarrenStrikeDismissing
    private let openContestPage: () -> Void

    init(
        source: @escaping () -> WarrenAccountStanding?,
        dismissStore: WarrenStrikeDismissing,
        openContestPage: @escaping () -> Void
    ) {
        self.source = source
        self.dismissStore = dismissStore
        self.openContestPage = openContestPage
        super.init()
    }

    override var identifier: NotificationProviderIdentifier {
        .warrenAccountStrikeInAppNotification
    }

    override var priority: NotificationPriority {
        .high
    }

    var notificationDescriptor: InAppNotificationDescriptor? {
        guard let notice = Self.shouldDisplay(
            standing: source(),
            dismissed: dismissStore.warrenDismissedStrikes
        ) else {
            return nil
        }
        let body = WarrenAccountStandingText.warning(notice) + " "
            + WarrenAccountStandingText.caseReference(notice.strike)
        return InAppNotificationDescriptor(
            identifier: identifier,
            style: .warning,
            title: String(localized: "PORT FORWARDING WARNING", table: "Settings"),
            body: NSAttributedString(string: body),
            button: InAppNotificationAction(
                image: UIImage.Buttons.closeSmall,
                handler: { [weak self] in
                    self?.dismiss(notice.strike.dismissalKey)
                }
            ),
            tapAction: InAppNotificationAction(
                handler: { [weak self] in
                    self?.openContestPage()
                }
            )
        )
    }

    /// Puts one strike's banner away. Append-only and de-duplicated.
    func dismiss(_ key: String) {
        var dismissed = dismissStore.warrenDismissedStrikes
        guard !dismissed.contains(key) else { return }
        dismissed.append(key)
        dismissStore.warrenDismissedStrikes = dismissed
        invalidate()
    }
}
