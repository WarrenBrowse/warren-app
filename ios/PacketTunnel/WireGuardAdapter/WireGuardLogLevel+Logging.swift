//
//  WireGuardLogLevel+Logging.swift
//  PacketTunnel
//
//  Created by pronebird on 15/07/2022.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import Foundation
import WarrenLogging
import WireGuardKit

extension WireGuardLogLevel {
    var loggerLevel: Logger.Level {
        switch self {
        case .verbose:
            return .debug
        case .error:
            return .error
        }
    }
}
