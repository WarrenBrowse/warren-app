//
//  NotificationSettingsViewModel.swift
//  MullvadVPN
//
//  Created by Mojgan on 2026-01-20.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import Combine
import WarrenLogging
import WarrenSettings
import SwiftUI
import UserNotifications

@MainActor
protocol NotificationSettingsViewModelProtocol: ObservableObject {
    var isNotificationsAllowed: Bool { get set }
    var isNotificationsDisabled: Bool { get set }
    var settings: NotificationSettings { get set }

    func binding(for key: NotificationKeys) -> Binding<Bool>
    func checkNotificationPermission()
    func openAppSettings()
    func enableNotifications()
}

final class NotificationSettingsViewModel: NotificationSettingsViewModelProtocol {
    @Published var isNotificationsAllowed: Bool = false
    @Published var isNotificationsDisabled: Bool = false
    @Published var settings: NotificationSettings = NotificationSettings()
    private var logger = Logger(label: "NotificationSettingsViewModel")

    init(settings: NotificationSettings) {
        self.settings = settings
    }

    func checkNotificationPermission() {
        Task { @MainActor in
            self.isNotificationsAllowed = await UNUserNotificationCenter.isAllowed
            self.isNotificationsDisabled = await UNUserNotificationCenter.isDisabled
        }
    }

    func enableNotifications() {
        if isNotificationsDisabled {
            openAppSettings()
        } else {
            requestNotificationPermission { isGranted in
                self.isNotificationsAllowed = isGranted
            }
        }
    }

    private func requestNotificationPermission(completion: @MainActor @Sendable @escaping (Bool) -> Void) {
        let options: UNAuthorizationOptions = [.alert, .sound, .badge]

        UNUserNotificationCenter
            .current()
            .requestAuthorization(options: options) { granted, error in
                Task { @MainActor in
                    if let error = error as NSError? {
                        self.logger.error(
                            error: error,
                            message: "Failed to obtain user notifications authorizations"
                        )
                        completion(false)
                    } else {
                        completion(true)
                    }

                }
            }
    }

    func openAppSettings() {
        if let url = URL(string: UIApplication.openNotificationSettingsURLString) {
            if UIApplication.shared.canOpenURL(url) {
                UIApplication.shared.open(url)
            }
        }
    }

    func binding(for key: NotificationKeys) -> Binding<Bool> {
        // The forum switch governs more than a banner: the header bell and the
        // app icon badge follow it too, and both work with system
        // notifications denied. Tying it to the system permission would take
        // the bell away from anyone who declined banners, which is a setting
        // they never touched.
        guard key.needsSystemPermission else {
            return Binding(
                get: { self.settings[key] },
                set: { self.settings[key] = $0 }
            )
        }
        return Binding(
            get: { self.settings[key] && self.isNotificationsAllowed },
            set: { self.settings[key] = $0 && self.isNotificationsAllowed }
        )
    }
}
