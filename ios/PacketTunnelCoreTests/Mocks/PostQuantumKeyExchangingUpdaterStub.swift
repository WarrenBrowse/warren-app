//
//  PostQuantumKeyExchangingUpdaterStub.swift
//  PacketTunnelCoreTests
//
//  Created by Mojgan on 2024-07-15.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import Foundation

@testable import WarrenTypes
@testable import PacketTunnelCore

final class PostQuantumKeyExchangingUpdaterStub: PostQuantumKeyExchangingUpdaterProtocol {
    var reconfigurationHandler: ConfigUpdater?
}
