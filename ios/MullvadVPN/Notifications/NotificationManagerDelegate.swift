//
//  NotificationManagerDelegate.swift
//  MullvadVPN
//
//  Created by pronebird on 09/12/2022.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import Foundation
import WarrenTypes

protocol NotificationManagerDelegate: AnyObject {
    func notificationManagerDidUpdateInAppNotifications(
        _ manager: NotificationManager,
        notifications: [InAppNotificationDescriptor]
    )

    func notificationManager(_ manager: NotificationManager, didReceiveResponse: NotificationResponse)
}
