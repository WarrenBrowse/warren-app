//
//  AppPreferences.swift
//  MullvadVPN
//
//  Created by Mojgan on 2023-08-09.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import Foundation

public protocol AppPreferencesDataSource {
    var hasDoneFirstTimeLaunch: Bool { get set }
    var hasDoneFirstTimeLogin: Bool { get set }
    var isShownOnboarding: Bool { get set }
    var isAgreedToTermsOfService: Bool { get set }
    var lastSeenChangeLogVersion: String { get set }
    var lastVersionCheck: VersionCheck { get set }
    var isNotificationPermissionAsked: Bool { get set }
    var notificationSettings: NotificationSettings { get set }
    var includeAllNetworksConsent: Bool { get set }
    var hasCompletedWarrenOnboarding: Bool { get set }
    var warrenAcknowledgedFailoverCount: Int { get set }
}

enum AppStorageKey: String {
    case hasDoneFirstTimeLaunch = "hasFinishedFirstTimeLaunch"
    case hasDoneFirstTimeLogin
    case isShownOnboarding
    case isAgreedToTermsOfService
    case lastSeenChangeLogVersion
    case lastVersionCheck
    case isNotificationPermissionAsked
    case notificationSettings
    case includeAllNetworksConsent
    case hasCompletedWarrenOnboarding
    case warrenAcknowledgedFailoverCount
}

public final class AppPreferences: AppPreferencesDataSource {
    public init() {}

    @PrimitiveStorage(key: AppStorageKey.hasDoneFirstTimeLaunch.rawValue, container: .standard)
    public var hasDoneFirstTimeLaunch: Bool = false

    @PrimitiveStorage(key: AppStorageKey.hasDoneFirstTimeLogin.rawValue, container: .standard)
    public var hasDoneFirstTimeLogin: Bool = false

    @PrimitiveStorage(key: AppStorageKey.isShownOnboarding.rawValue, container: .standard)
    public var isShownOnboarding = true

    @PrimitiveStorage(key: AppStorageKey.isAgreedToTermsOfService.rawValue, container: .standard)
    public var isAgreedToTermsOfService = false

    @PrimitiveStorage(key: AppStorageKey.lastSeenChangeLogVersion.rawValue, container: .standard)
    public var lastSeenChangeLogVersion = ""

    @CompositeStorage(key: AppStorageKey.lastVersionCheck.rawValue, container: .standard)
    public var lastVersionCheck = VersionCheck(version: "", date: .distantPast)

    @PrimitiveStorage(key: AppStorageKey.isNotificationPermissionAsked.rawValue, container: .standard)
    public var isNotificationPermissionAsked = false

    @CompositeStorage(key: AppStorageKey.notificationSettings.rawValue, container: .standard)
    public var notificationSettings = NotificationSettings()

    @PrimitiveStorage(key: AppStorageKey.includeAllNetworksConsent.rawValue, container: .standard)
    public var includeAllNetworksConsent = false

    /// Set to `true` once the Warren onboarding wizard has run to completion.
    /// Reset on logout so re-login (or fresh wallet provisioning) replays the
    /// flow. Persisted in UserDefaults.standard, NOT in the App Group, because
    /// it tracks UI state that does not need to be shared with the packet
    /// tunnel extension.
    @PrimitiveStorage(key: AppStorageKey.hasCompletedWarrenOnboarding.rawValue, container: .standard)
    public var hasCompletedWarrenOnboarding = false

    /// The multi-exit failover count the user has already acknowledged by
    /// dismissing the "EXIT SWITCHED" banner. The banner reappears only when
    /// the live count (broadcast by the packet tunnel extension) exceeds this
    /// high-water mark, so a dismissed banner stays dismissed across launches
    /// until another failover lands.
    @PrimitiveStorage(key: AppStorageKey.warrenAcknowledgedFailoverCount.rawValue, container: .standard)
    public var warrenAcknowledgedFailoverCount: Int = 0
}
