//
//  WarrenForumActivityRow.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  What one row of the forum activity panel says and how old it reads. The
//  desktop `forum-activity/helpers.ts`, wording for wording, so the two
//  panels describe the same event the same way.
//

import Foundation
import WarrenRustRuntime

public enum WarrenForumActivityRow {
    /// Glyph that lets the eye sort a reply from a like before reading a
    /// word. SF Symbols, which is this platform's answer to the desktop icon
    /// set.
    public static func symbol(for kind: WarrenForumNotificationKind) -> String {
        switch kind {
        case .liked: "heart"
        case .privateMessage: "envelope"
        case .mentioned, .quoted: "person"
        case .grantedBadge: "checkmark.seal"
        case .announcement: "info.circle"
        case .linked: "link"
        case .replied, .posted, .watchingFirstPost: "arrowshape.turn.up.left"
        case .other: "bell"
        }
    }

    /// One line saying who did what, falling back when the forum said less.
    public static func headline(for notification: WarrenForumNotification) -> String {
        let actor =
            notification.actor
            ?? NSLocalizedString(
                "Someone",
                tableName: "Settings",
                comment: ""
            )

        switch notification.kind {
        case .replied, .posted:
            return fill("%@ replied", actor)
        case .liked:
            return fill("%@ liked your post", actor)
        case .mentioned:
            return fill("%@ mentioned you", actor)
        case .quoted:
            return fill("%@ quoted you", actor)
        case .privateMessage:
            return fill("%@ sent you a message", actor)
        case .linked:
            return fill("%@ linked to your post", actor)
        case .watchingFirstPost:
            return fill("%@ opened a new topic", actor)
        case .grantedBadge:
            return NSLocalizedString("You earned a badge", tableName: "Settings", comment: "")
        case .announcement:
            return NSLocalizedString("The forum was updated", tableName: "Settings", comment: "")
        case .other:
            return NSLocalizedString("New forum activity", tableName: "Settings", comment: "")
        }
    }

    /// Compact age, "2 h ago" while that is still the useful thing to say and
    /// a plain date past a week, where "5 weeks ago" tells a reader less than
    /// the day it happened.
    ///
    /// `RelativeDateTimeFormatter` rather than translated strings: it already
    /// knows every locale's plural forms and its own wording, so the list
    /// reads naturally without the app shipping a plural rule per unit per
    /// language.
    public static func age(of createdAt: Date, now: Date = Date(), locale: Locale = .current) -> String {
        guard WarrenForumActivity.ageIsRelative(createdAt: createdAt, now: now) else {
            let formatter = DateFormatter()
            formatter.locale = locale
            formatter.dateStyle = .medium
            formatter.timeStyle = .none
            return formatter.string(from: createdAt)
        }
        let formatter = RelativeDateTimeFormatter()
        formatter.locale = locale
        formatter.unitsStyle = .short
        formatter.dateTimeStyle = .numeric
        return formatter.localizedString(for: createdAt, relativeTo: now)
    }

    /// The forum page a row opens, or `nil` when it points at nothing
    /// openable. The path was already held to the shapes the forum produces
    /// in Rust, so nothing here can climb out of the forum origin.
    public static func url(for notification: WarrenForumNotification) -> URL? {
        guard let path = notification.path else { return nil }
        return URL(string: WarrenProductAnchors.current.forumPublicURL + path)
    }

    private static func fill(_ key: String, _ actor: String) -> String {
        String(format: NSLocalizedString(key, tableName: "Settings", comment: ""), actor)
    }
}
