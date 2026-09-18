//
//  WarrenForumActivityAlert.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  The local notification about new forum activity, and its life.
//
//  The count is all the broadcast digest carries. Nothing here names a topic
//  or an author, which is what keeps the badge free of any per-user request;
//  the content is only ever read when the user opens the panel.
//

import Foundation
import UserNotifications

public enum WarrenForumActivityAlert {
    /// One identifier, so a rise replaces the previous notification instead
    /// of stacking a second one saying the same thing.
    public static let identifier = "warren-forum-activity"

    /// What the notification says for `unread`, as a pure function so the
    /// wording is tested without a notification centre.
    public static func message(unread: Int) -> String {
        switch WarrenForumActivity.wording(unread: unread) {
        case .single:
            return NSLocalizedString(
                "New notification on the forum",
                tableName: "Settings",
                comment: ""
            )
        case let .several(count):
            return String(
                format: NSLocalizedString(
                    "%d new notifications on the forum",
                    tableName: "Settings",
                    comment: ""
                ),
                count
            )
        case let .moreThan(count):
            return String(
                format: NSLocalizedString(
                    "More than %d new notifications on the forum",
                    tableName: "Settings",
                    comment: ""
                ),
                count
            )
        }
    }

    /// Posts the notification for a rise this run was watching.
    public static func post(unread: Int) {
        guard unread > 0 else { return }
        let content = UNMutableNotificationContent()
        content.title = NSLocalizedString("Forum", tableName: "Settings", comment: "")
        content.body = message(unread: unread)
        // Passive on purpose: this is the band the system notification
        // setting is allowed to suppress, so turning notifications off
        // silences it too, and the forum setting is an extra gate on top
        // rather than a way around it.
        content.interruptionLevel = .passive

        let request = UNNotificationRequest(
            identifier: identifier,
            content: content,
            trigger: nil
        )
        UNUserNotificationCenter.current().add(request)
    }

    /// Nothing is waiting any more: whatever was posted comes down.
    public static func clear() {
        UNUserNotificationCenter.current()
            .removeDeliveredNotifications(withIdentifiers: [identifier])
        UNUserNotificationCenter.current()
            .removePendingNotificationRequests(withIdentifiers: [identifier])
    }
}
