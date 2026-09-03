//
//  AppPreferences.swift
//  MullvadVPN
//
//  Created by Mojgan on 2023-08-09.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import Foundation

/// What this build gave up when a higher-priority product environment took
/// the device, and whether the user has asked for it back.
///
/// Persisted because the mobile rule is a TRANSITION, not a live state: the
/// other install is still there at every later launch, so a stand-down that
/// re-decided from scratch would tear this build down again and undo the
/// manual re-enable every time the app starts.
public struct WarrenEnvStandDownRecord: Codable, Equatable, Sendable {
    /// The environment last observed on the device (`prod`, `staging`), `nil`
    /// while none has been seen. Dropped whole when that install goes away,
    /// so a later reinstall reads as the new transition it is.
    public var higherEnvironmentSeen: String?

    /// The user asked for this build back. Sticky by construction: the
    /// environment stays marked as seen, so no later launch stands down for
    /// the same install again.
    public var reEnabled: Bool

    /// The "Force all apps" state to put back on a manual re-enable, snapshot
    /// before the stand-down turned it off.
    public var restoreIncludeAllNetworks: Bool

    /// Whether the stand-down banner is up: an environment was seen and the
    /// user has not overridden it.
    public var isStandingDown: Bool { higherEnvironmentSeen != nil && !reEnabled }

    public init(
        higherEnvironmentSeen: String? = nil,
        reEnabled: Bool = false,
        restoreIncludeAllNetworks: Bool = false
    ) {
        self.higherEnvironmentSeen = higherEnvironmentSeen
        self.reEnabled = reEnabled
        self.restoreIncludeAllNetworks = restoreIncludeAllNetworks
    }
}

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
    case warrenEnvStandDown
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

    /// The coexistence record: which higher-priority environment took this
    /// device, what this build gave up for it, and whether the user asked for
    /// this build back anyway.
    @CompositeStorage(key: AppStorageKey.warrenEnvStandDown.rawValue, container: .standard)
    public var warrenEnvStandDown = WarrenEnvStandDownRecord()
}
