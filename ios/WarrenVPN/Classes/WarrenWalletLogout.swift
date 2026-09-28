//
//  WarrenWalletLogout.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import Foundation
import WarrenLogging

/// True sign-out for the wallet identity model: takes the tunnel down, then
/// wipes the wallet from the Keychain. Unlike the Settings "Erase wallet"
/// flow it does NOT reset `hasCompletedWarrenOnboarding`, so the next launch
/// routes to the wallet login screen (Create / Restore) rather than the full
/// onboarding wizard, matching the desktop logout behavior.
enum WarrenWalletLogout {
    @MainActor static func perform(tunnelManager: TunnelManager) async {
        let logger = Logger(label: "WarrenWalletLogout")
        await takeTunnelDown(tunnelManager: tunnelManager)
        do {
            try WarrenWalletKeychain.delete()
        } catch {
            logger.error("Failed to delete wallet on logout: \(error)")
        }
        // The port-forward standing of the wallet that left goes with it, on
        // screen and on disk, rather than at the next poll.
        WarrenAccountStandingFeed.current?.walletDidLeave()
        // No browsing-history store exists yet; clear it here once one lands.
    }

    /// A wallet leaving the device leaves no tunnel behind: every path that
    /// erases the wallet runs this first. Stopping clears On Demand, so the
    /// system cannot relaunch the extension for a wallet that is gone;
    /// unsetting the account then removes the VPN configuration and marks the
    /// device logged out.
    @MainActor static func takeTunnelDown(tunnelManager: TunnelManager) async {
        await withCheckedContinuation { continuation in
            tunnelManager.stopTunnel { _ in continuation.resume() }
        }
        await tunnelManager.unsetAccount(isRemovingProfile: true)
    }
}
