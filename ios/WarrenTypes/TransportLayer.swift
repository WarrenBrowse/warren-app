//
//  TransportLayer.swift
//  WarrenTypes
//
//  Created by Marco Nikic on 2023-11-24.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import Foundation

public enum TransportLayer: Codable, Sendable {
    case udp
    case tcp

    public var name: String {
        switch self {
        case .udp:
            NSLocalizedString("UDP", comment: "")
        case .tcp:
            NSLocalizedString("TCP", comment: "")
        }
    }
}
