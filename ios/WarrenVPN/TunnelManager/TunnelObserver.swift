//
//  TunnelObserver.swift
//  MullvadVPN
//
//  Created by pronebird on 19/08/2021.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import Foundation
import WarrenSettings

protocol TunnelObserver: AnyObject, Sendable {
    func tunnelManagerDidLoadConfiguration(_ manager: TunnelManager)
    func tunnelManager(_ manager: TunnelManager, didUpdateTunnelStatus tunnelStatus: TunnelStatus)
    func tunnelManager(
        _ manager: TunnelManager,
        didUpdateDeviceState deviceState: DeviceState,
        previousDeviceState: DeviceState
    )
    func tunnelManager(_ manager: TunnelManager, didUpdateTunnelSettings tunnelSettings: LatestTunnelSettings)
    func tunnelManager(_ manager: TunnelManager, didFailWithError error: Error)
}
