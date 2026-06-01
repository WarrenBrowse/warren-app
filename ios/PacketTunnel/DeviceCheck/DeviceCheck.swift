//
//  DeviceCheck.swift
//  PacketTunnel
//
//  Created by pronebird on 13/09/2023.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import Foundation
import WarrenTypes

/// The verdict of an account status check.
enum AccountVerdict: Equatable {
    /// Account is no longer valid.
    case invalid

    /// Account is expired.
    case expired(Account)

    /// Account exists and has enough time left.
    case active(Account)
}

/// The verdict of a device status check.
enum DeviceVerdict: Equatable {
    /// Device is revoked.
    case revoked

    /// Device is in good standing and should work as normal.
    case active
}

/// Struct holding data associated with account and device diagnostics performed by packet tunnel process.
struct DeviceCheck: Equatable {
    /// The verdict of account status check.
    var accountVerdict: AccountVerdict

    /// The verdict of device status check.
    var deviceVerdict: DeviceVerdict
}
