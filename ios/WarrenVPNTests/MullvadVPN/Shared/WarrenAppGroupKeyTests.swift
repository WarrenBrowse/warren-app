//
//  WarrenAppGroupKeyTests.swift
//  WarrenVPNTests
//
//  Created by Warren on 2026-05-22.
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  Coverage tests for `WarrenAppGroupKey` (Shared/). Ensures the
//  rawValue strings stay stable across refactors (silent rename
//  would break cross-process App Group reads since the PacketTunnel
//  extension and main app compile separately).
//

import XCTest

@testable import WarrenVPN

final class WarrenAppGroupKeyTests: XCTestCase {
    /// All 4 declared keys are present in `allCases`. Adding a 5th
    /// key without updating consumers (WarrenAppGroupEvents observer +
    /// WarrenQuinnTunnelImplementation.broadcastEvent) is a bug ; this
    /// test fails fast so the omission is caught at build time.
    func test_allCases_includesAllExpectedKeys() {
        let cases = WarrenAppGroupKey.allCases
        XCTAssertEqual(cases.count, 4, "Add the new key to consumers before expanding allCases")
        XCTAssertTrue(cases.contains(.lastFailoverExit))
        XCTAssertTrue(cases.contains(.lastFailoverAt))
        XCTAssertTrue(cases.contains(.obfuscationActive))
        XCTAssertTrue(cases.contains(.natPmpExternalPort))
    }

    /// Raw values follow the `WarrenTunnel.<field>` convention so they
    /// cluster in the App Group inspector and don't collide with
    /// Mullvad-fork keys.
    func test_rawValues_followConvention() {
        for key in WarrenAppGroupKey.allCases {
            XCTAssertTrue(
                key.rawValue.hasPrefix("WarrenTunnel."),
                "Key \(key) raw value \(key.rawValue) must start with WarrenTunnel."
            )
        }
    }

    /// The exact raw value strings — pinning prevents silent renames.
    /// If a key needs to be renamed, update this test AND every
    /// producer/consumer in lock-step.
    func test_rawValues_exactStrings() {
        XCTAssertEqual(WarrenAppGroupKey.lastFailoverExit.rawValue, "WarrenTunnel.lastFailoverExit")
        XCTAssertEqual(WarrenAppGroupKey.lastFailoverAt.rawValue, "WarrenTunnel.lastFailoverAt")
        XCTAssertEqual(WarrenAppGroupKey.obfuscationActive.rawValue, "WarrenTunnel.obfuscationActive")
        XCTAssertEqual(WarrenAppGroupKey.natPmpExternalPort.rawValue, "WarrenTunnel.natPmpExternalPort")
    }

    /// No two keys share a raw value (would silently overwrite each
    /// other in UserDefaults).
    func test_rawValues_areUnique() {
        let rawValues = WarrenAppGroupKey.allCases.map(\.rawValue)
        let uniqueRawValues = Set(rawValues)
        XCTAssertEqual(rawValues.count, uniqueRawValues.count, "Duplicate raw values found in WarrenAppGroupKey")
    }
}
