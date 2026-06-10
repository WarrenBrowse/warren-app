//
//  LoggedBuilderTests.swift
//  MullvadVPNTests
//
//  Created by Marco Nikic on 2025-11-04.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import Testing

@testable import WarrenLogging
@testable import WarrenVPN

struct LoggerBuilderTests {

    @Test func installIsIdempotent() async throws {
        LoggerBuilder.shared.install()
        // This should crash if the `install` function is not idempotent
        LoggerBuilder.shared.install()
        LoggerBuilder.shared.install()
    }
}
