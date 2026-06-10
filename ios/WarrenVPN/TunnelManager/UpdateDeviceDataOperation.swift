//
//  UpdateDeviceDataOperation.swift
//  MullvadVPN
//
//  Created by pronebird on 13/05/2022.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import Foundation
import WarrenLogging
import WarrenSettings
import WarrenTypes
import Operations

/// Warren has no Mullvad device registration: the wallet pubkey is the
/// identity and the exit enforces access via its allowlist. The
/// synthesized device data is static, so there is nothing to refresh from
/// the backend. This operation therefore returns the stored device data
/// unchanged instead of calling the Mullvad `getDevice` endpoint, which
/// warren-api does not serve and which would surface a spurious
/// `deviceNotFound` error that wrongly revokes the device.
class UpdateDeviceDataOperation: ResultOperation<StoredDeviceData>, @unchecked Sendable {
    private let interactor: TunnelInteractor

    init(
        dispatchQueue: DispatchQueue,
        interactor: TunnelInteractor
    ) {
        self.interactor = interactor

        super.init(dispatchQueue: dispatchQueue)
    }

    override func main() {
        guard case let .loggedIn(_, deviceData) = interactor.deviceState else {
            finish(result: .failure(InvalidDeviceStateError()))
            return
        }
        finish(result: .success(deviceData))
    }
}
