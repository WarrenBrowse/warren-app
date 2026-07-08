//
//  NetworkPath+.swift
//  PacketTunnelCore
//
//  Created by pronebird on 14/09/2023.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import Foundation
import Network

extension Network.NWPath.Status {
    /// Converts `NetworkPath.status` into `NetworkReachability`.
    public var networkReachability: NetworkReachability {
        switch self {
        case .satisfied:
            .reachable
        case .unsatisfied:
            .unreachable
        case .requiresConnection:
            .reachable
        @unknown default:
            .undetermined
        }
    }

    /// The status as seen by the Warren tunnel: relays are dialed over IPv4,
    /// so a path that is satisfied but cannot carry IPv4 cannot carry the
    /// tunnel either and must read as unsatisfied. Keeps the reconnect
    /// trigger and the offline treatment from churning on v6-only edges
    /// (the family gating the desktop applies in its error state).
    public func warrenEffectiveStatus(supportsIPv4: Bool) -> Network.NWPath.Status {
        if case .satisfied = self, !supportsIPv4 {
            return .unsatisfied
        }
        return self
    }
}
