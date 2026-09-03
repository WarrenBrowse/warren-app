//
//  WarrenAnnouncementNotificationProvider.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import Foundation
import WarrenRustRuntime
import WarrenTypes
import UIKit.UIImage

/// The operator's launch announcement on the connect screen.
///
/// The banner slot carries a title, a plain body and one close button, which a
/// headline, a body, a 16 character code and a link cannot share. So the
/// banner carries the compact entry (the operator's headline, a lead, and the
/// invitation to read the rest) and tapping it opens the announcement in full,
/// where the code can be selected and copied and the link followed.
///
/// A dismissal is by announcement id and it is permanent: the card is an event
/// rather than a live statement, and it may already have handed over a code, so
/// raising it again on every launch would be nagging about something the reader
/// has dealt with.
///
/// Nothing is filtered here beyond the dismissal. Whether an announcement may
/// be displayed at all was settled in Rust against the pinned server key, so an
/// empty set means exactly "there is nothing to show", whether it was
/// withdrawn, lapsed, or never verified.
final class WarrenAnnouncementNotificationProvider: NotificationProvider,
    InAppNotificationProvider, @unchecked Sendable
{
    /// How much of the operator's body the banner carries before the reader is
    /// sent to the full announcement. The banner's body label wraps without a
    /// line limit, so an unbounded body would push the connect card off the
    /// screen; the whole text is one tap away and nothing is lost.
    static let bannerLeadLimit = 140

    /// The announcement to display, `nil` when there is none.
    ///
    /// The set arrives in publication order and the first one not yet put away
    /// is the one shown: reordering it here would make which card a reader sees
    /// depend on a rule nobody publishing one can see.
    static func shouldDisplay(
        announcements: [WarrenAnnouncement],
        dismissed: [String]
    ) -> WarrenAnnouncement? {
        announcements.first { !dismissed.contains($0.id) }
    }

    /// The operator's body, cut at a word boundary when it is longer than
    /// `limit`. Returns it untouched when it fits, so a short announcement
    /// reads exactly as written.
    static func bannerLead(_ body: String, limit: Int = bannerLeadLimit) -> String {
        guard body.count > limit else { return body }
        let head = body.prefix(limit)
        let cut = head.lastIndex(of: " ").map { head[head.startIndex..<$0] } ?? head
        return String(cut).trimmingCharacters(in: .whitespacesAndNewlines) + "\u{2026}"
    }

    /// The banner indicator for an announcement's level, the same mapping the
    /// desktop card uses: the banner has three tiers and the wire has an
    /// informational one, which reads as the calm tier rather than as a
    /// warning.
    static func style(for level: WarrenAnnouncementLevel) -> NotificationBannerStyle {
        switch level {
        case .error: return .error
        case .warning: return .warning
        case .info: return .success
        }
    }

    private let source: () -> [WarrenAnnouncement]
    private let dismissStore: WarrenAnnouncementDismissing
    private let present: (WarrenAnnouncement) -> Void

    init(
        source: @escaping () -> [WarrenAnnouncement],
        dismissStore: WarrenAnnouncementDismissing,
        present: @escaping (WarrenAnnouncement) -> Void
    ) {
        self.source = source
        self.dismissStore = dismissStore
        self.present = present
        super.init()
    }

    override var identifier: NotificationProviderIdentifier {
        .warrenAnnouncementInAppNotification
    }

    override var priority: NotificationPriority {
        .high
    }

    var notificationDescriptor: InAppNotificationDescriptor? {
        guard let announcement = Self.shouldDisplay(
            announcements: source(),
            dismissed: dismissStore.warrenDismissedAnnouncements
        ) else {
            return nil
        }

        let body = [
            Self.bannerLead(announcement.body),
            NSLocalizedString("Read the full announcement.", comment: ""),
        ]
        .filter { !$0.isEmpty }
        .joined(separator: "\n")

        return InAppNotificationDescriptor(
            identifier: identifier,
            style: Self.style(for: announcement.level),
            // The operator's own headline IS the title. Stacking a level word
            // on top of it would demote the words they wrote to a subtitle and
            // say nothing the indicator does not already say.
            title: announcement.headline,
            body: NSAttributedString(string: body),
            button: InAppNotificationAction(
                image: UIImage.Buttons.closeSmall,
                handler: { [weak self] in
                    guard let self else { return }
                    dismiss(announcement.id)
                }
            ),
            tapAction: InAppNotificationAction(
                handler: { [weak self] in
                    self?.present(announcement)
                }
            )
        )
    }

    /// Puts one announcement away for good. Append-only and de-duplicated: the
    /// same id dismissed on two runs must not grow the stored list forever.
    func dismiss(_ id: String) {
        var dismissed = dismissStore.warrenDismissedAnnouncements
        guard !dismissed.contains(id) else { return }
        dismissed.append(id)
        dismissStore.warrenDismissedAnnouncements = dismissed
        invalidate()
    }
}
