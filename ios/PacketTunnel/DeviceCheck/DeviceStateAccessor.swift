//
//  DeviceStateAccessor.swift
//  PacketTunnel
//
//  Created by pronebird on 30/05/2023.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import Foundation
import WarrenSettings
import WarrenTypes

/// An object that provides access to `DeviceState` used by `DeviceCheckOperation`.
struct DeviceStateAccessor: DeviceStateAccessorProtocol {
    func read() throws -> DeviceState {
        try SettingsManager.readDeviceState()
    }

    func write(_ deviceState: DeviceState) throws {
        try SettingsManager.writeDeviceState(deviceState)
    }
}
