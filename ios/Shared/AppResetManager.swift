//
//  AppResetManager.swift
//  MullvadVPN
//
//  Created by Mojgan on 2026-02-23.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import WarrenLogging
import WarrenSettings
import NetworkExtension
import UIKit

@MainActor
final class AppResetManager {
    private let launchArguments: LaunchArguments
    private let tunnelManager: TunnelManager
    private var tunnelObserver: TunnelObserver!
    let logger = Logger(label: "AppResetManager")

    var onAppReady: (@Sendable @MainActor () -> Void)?

    init(
        launchArguments: LaunchArguments,
        tunnelManager: TunnelManager
    ) {
        self.launchArguments = launchArguments
        self.tunnelManager = tunnelManager
        guard launchArguments.target.isUITest else { return }
        addObserver()
        Task {
            await setup()
        }
    }

    private func addObserver() {
        let tunnelObserver = TunnelBlockObserver(
            didUpdateTunnelStatus: { [weak self] tunnelManager, tunnelStatus in
                guard let self else { return }
                if tunnelStatus.observedState != .disconnected {
                    tunnelManager.stopTunnel()
                } else if case .disconnected = tunnelStatus.observedState {
                    Task {
                        await reset()
                    }
                }
            }
        )
        tunnelManager.addObserver(tunnelObserver)
        self.tunnelObserver = tunnelObserver
    }

    private func setup() async {
        do {
            guard try await isTunnelActive() == false else { return }
            await reset()
        } catch {
            logger.error("Unexpected tunnel error: \(error.localizedDescription)")
            onAppReady?()
        }
    }

    private func reset() async {
        defer { tunnelManager.removeObserver(tunnelObserver!) }
        switch tunnelManager.deviceState {
        case .loggedIn:
            await logoutIfNeeded()
            fallthrough
        default:
            resetUserDefaults()
            resetKeychain()
            onAppReady?()
        }
    }

    private func logoutIfNeeded() async {
        guard launchArguments.authenticationState == .forceLoggedOut else {
            return
        }
        await tunnelManager.unsetAccount(isRemovingProfile: false)
    }

    private func resetKeychain() {
        let policy = launchArguments.settingsResetPolicy
        SettingsManager.resetStore(policy: policy.toSettingsResetPolicy)
        if policy.shouldReset(.settings) {
            tunnelManager.updateSettings([.reset])
            // The BIP39 wallet is the Warren identity (the old Mullvad
            // account number's replacement), so wipe it alongside the
            // settings store. Without this, a wallet provisioned by a
            // prior UI test run would persist in the Keychain and the
            // app would route straight to .main instead of the login
            // chooser, making wallet-flow UI tests non-deterministic.
            try? WarrenWalletKeychain.delete()
        }
    }

    private func resetUserDefaults() {
        let policy = launchArguments.appPreferencesResetPolicy
        let defaults = UserDefaults.standard
        let keysToRemove: Set<UITestAppPreferencesKey> = policy.resolvedKeys()
        for key in keysToRemove {
            defaults.removeObject(forKey: key.rawValue)
        }
        // Pre-seed (set to true) AFTER the reset so a test can express a
        // state the reset-only policy cannot (e.g. onboarding already
        // complete). UITest preference keys share their raw values with
        // `AppStorageKey`, so writing the raw value is what the app reads.
        for key in launchArguments.appPreferencesSeed {
            defaults.set(true, forKey: key.rawValue)
        }
        defaults.synchronize()
    }

    private func isTunnelActive() async throws -> Bool {
        #if targetEnvironment(simulator)
            return false
        #else
            try await withCheckedThrowingContinuation { continuation in
                NETunnelProviderManager.loadAllFromPreferences { managers, error in
                    if let error {
                        continuation.resume(throwing: error)
                        return
                    }

                    let active = (managers ?? []).contains {
                        [.connected, .connecting, .reasserting].contains($0.connection.status)
                    }

                    continuation.resume(returning: active)
                }
            }
        #endif
    }
}

extension UITestSettingsKey {
    var toDomain: SettingsKey {
        switch self {
        case .settings: return .settings
        case .ipOverrides: return .ipOverrides
        case .customRelayLists: return .customRelayLists
        case .recentConnections: return .recentConnections
        }
    }
}

private extension UITestSettingsResetPolicy {
    var toSettingsResetPolicy: SettingsResetPolicy {
        switch self {
        case .none:
            .none
        case .allExcept(let keys):
            .allExcept(Set(keys.map(\.toDomain)))
        case .only(let keys):
            .only(Set(keys.map(\.toDomain)))
        case .all:
            .all
        }
    }
}
