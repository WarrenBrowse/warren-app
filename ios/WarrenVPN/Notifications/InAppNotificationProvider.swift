//
//  InAppNotificationProvider.swift
//  MullvadVPN
//
//  Created by pronebird on 09/12/2022.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import Foundation
import WarrenTypes

/// Protocol describing in-app notification provider.
protocol InAppNotificationProvider: NotificationProviderProtocol {
    /// In-app notification descriptor.
    var notificationDescriptor: InAppNotificationDescriptor? { get }
}
