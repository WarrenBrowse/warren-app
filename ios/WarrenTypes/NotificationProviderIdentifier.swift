//
//  NotificationProviderIdentifier.swift
//  MullvadVPN
//
//  Created by Mojgan on 2023-05-10.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import Foundation

public enum NotificationPriority: Int, Comparable {
    case low = 1
    case medium = 2
    case high = 3
    case critical = 4
    /// Above everything, because only the highest-priority descriptor reaches
    /// the banner (`NotificationController.setNotifications` renders
    /// `first`). Reserved for the case where every other message would
    /// describe a tunnel this build no longer holds.
    case exclusive = 5

    public static func < (lhs: NotificationPriority, rhs: NotificationPriority) -> Bool {
        lhs.rawValue < rhs.rawValue
    }
}

public enum NotificationProviderIdentifier: String {
    case accountExpirySystemNotification = "AccountExpiryNotification"
    case newAppVersionSystemNotification = "NewAppVersionSystemNotification"
    case newAppVersionInAppNotification = "NewAppVersionInAppNotification"
    case accountExpiryInAppNotification = "AccountExpiryInAppNotification"
    case tunnelStatusNotificationProvider = "TunnelStatusNotificationProvider"
    case latestChangesInAppNotificationProvider = "LatestChangesInAppNotificationProvider"
    case warrenFailoverInAppNotification = "WarrenFailoverInAppNotification"
    case warrenEnvStandDownInAppNotification = "WarrenEnvStandDownInAppNotification"
    case warrenAnnouncementInAppNotification = "WarrenAnnouncementInAppNotification"
    case `default` = "default"

    public var domainIdentifier: String {
        "com.warrenbrowse.vpn.ios.\(rawValue)"
    }
}
