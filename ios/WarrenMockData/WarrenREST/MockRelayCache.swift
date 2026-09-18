//
//  MockRelayCache.swift
//  MullvadVPN
//
//  Created by Mojgan on 2025-03-10.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import Foundation

@testable import WarrenREST

public struct MockRelayCache: RelayCacheProtocol {
    /// Whether `read()` answers with a list at all. A cache that has never been
    /// filled is the state the shuffle button has to stay disabled in.
    public var isEmpty: Bool

    public init(isEmpty: Bool = false) {
        self.isEmpty = isEmpty
    }

    public func read() throws -> WarrenREST.CachedRelays {
        CachedRelays(
            relays: isEmpty ? ServerRelaysResponseStubs.emptyRelays : ServerRelaysResponseStubs.sampleRelays,
            updatedAt: Date()
        )
    }

    public func readPrebundledRelays() throws -> WarrenREST.CachedRelays {
        try self.read()
    }

    public func write(record: WarrenREST.StoredRelays) throws {}
}
