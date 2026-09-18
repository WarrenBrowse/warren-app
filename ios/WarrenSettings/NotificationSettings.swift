//
//  NotificationSettings.swift
//  MullvadVPN
//
//  Created by Mojgan on 2026-01-20.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//
import WarrenTypes

public enum NotificationKeys: String, CaseIterable {
    case account
    case forumActivity

    var keyPath: KeyPath<NotificationSettings, Bool> {
        switch self {
        case .account:
            \.isAccountNotificationEnabled
        case .forumActivity:
            \.isForumActivityNotificationEnabled
        }
    }

    /// Whether the switch only governs something the system may suppress. The
    /// forum switch also drives the header bell and the app icon badge, which
    /// a denied notification permission does not reach.
    public var needsSystemPermission: Bool {
        switch self {
        case .account:
            true
        case .forumActivity:
            false
        }
    }

    var writableKeyPath: WritableKeyPath<NotificationSettings, Bool> {
        switch self {
        case .account:
            \.isAccountNotificationEnabled
        case .forumActivity:
            \.isForumActivityNotificationEnabled
        }
    }
}

public struct NotificationSettings: Codable, Sendable, Equatable {
    public var isAccountNotificationEnabled: Bool

    /// Whether the app shows community-forum activity at all: the header
    /// bell, the local notification and the app icon badge alike. Off means
    /// off everywhere, and it also stops the digest being fetched, since
    /// nothing on this installation would then read it.
    public var isForumActivityNotificationEnabled: Bool

    public init(
        isAccountNotificationEnabled: Bool = true,
        isForumActivityNotificationEnabled: Bool = true
    ) {
        self.isAccountNotificationEnabled = isAccountNotificationEnabled
        self.isForumActivityNotificationEnabled = isForumActivityNotificationEnabled
    }

    /// A record written before the forum switch existed carries no value for
    /// it. Decoding that as `false` would turn a feature off for everyone who
    /// upgraded, so the missing field reads as the default the switch ships
    /// in.
    public init(from decoder: any Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        isAccountNotificationEnabled = try container.decode(
            Bool.self,
            forKey: .isAccountNotificationEnabled
        )
        isForumActivityNotificationEnabled =
            try container.decodeIfPresent(
                Bool.self,
                forKey: .isForumActivityNotificationEnabled
            ) ?? true
    }

    public subscript(key: NotificationKeys) -> Bool {
        get {
            self[keyPath: key.keyPath]
        }
        set {
            self[keyPath: key.writableKeyPath] = newValue
        }
    }

    public var allAreEnabled: Bool {
        NotificationKeys.allCases.allSatisfy { self[$0] }
    }
}
