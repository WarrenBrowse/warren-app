//
//  WarrenWalletLogout.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import Foundation
import WarrenLogging

/// True sign-out for the wallet identity model: wipes the wallet from the
/// Keychain and resets the device state. Unlike the Settings "Erase wallet"
/// flow it does NOT reset `hasCompletedWarrenOnboarding`, so the next launch
/// routes to the wallet login screen (Create / Restore) rather than the full
/// onboarding wizard, matching the desktop logout behavior.
enum WarrenWalletLogout {
    @MainActor static func perform(tunnelManager: TunnelManager) {
        let logger = Logger(label: "WarrenWalletLogout")
        do {
            try WarrenWalletKeychain.delete()
        } catch {
            logger.error("Failed to delete wallet on logout: \(error)")
        }
        // No browsing-history store exists yet; clear it here once one lands.
        tunnelManager.setDeviceState(.loggedOut, persist: true)
    }
}
