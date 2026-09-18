//
//  WarrenNetworkInfoTests.swift
//  WarrenRustRuntimeTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import XCTest

@testable import WarrenRustRuntime

/// What Swift makes of the `GET /v1/network` envelope. Pure decoding, so it
/// runs off any device and off any network.
final class WarrenNetworkInfoTests: XCTestCase {
    func testAnEnvironmentDescriptorIsReadWholeWhenTheServerGaveOne() throws {
        let info = try XCTUnwrap(
            WarrenNetworkInfoClient.info(
                fromEnvelope: """
                    {"ok":true,"environment":"beta","degraded":true,
                     "default_rate_bps":20000000,"payments_enabled":false}
                    """))

        XCTAssertEqual(info.environment, "beta")
        XCTAssertTrue(info.degraded)
        XCTAssertEqual(info.defaultRateBps, 20_000_000)
        XCTAssertFalse(info.paymentsEnabled)
    }

    /// An API that predates the endpoint answers the same "no info" as a
    /// transport failure, and neither may be shown as a fact about the
    /// network.
    func testAnythingButAnExplicitOkIsNoInfo() {
        for envelope in [
            nil, "", "not json", #"{"ok":false}"#,
            // An `ok` with no environment is a half-read envelope, not a
            // network this app can name.
            #"{"ok":true,"degraded":true}"#,
        ] {
            XCTAssertNil(WarrenNetworkInfoClient.info(fromEnvelope: envelope), envelope ?? "nil")
        }
    }

    /// The cap is stated in whole megabits per second, which is the unit the
    /// badge and the dialog both use.
    func testTheCapIsRoundedToWholeMegabitsPerSecond() {
        let info = { (bps: UInt64?) in
            WarrenNetworkInfo(
                environment: "beta", degraded: true, defaultRateBps: bps, paymentsEnabled: false)
        }
        XCTAssertEqual(info(20_000_000).capMbps, 20)
        XCTAssertEqual(info(20_400_000).capMbps, 20)
        XCTAssertEqual(info(20_600_000).capMbps, 21)
    }

    /// A network with no cap says "limited", never "0 Mbps": a figure the
    /// user can check and find wrong is worse than none.
    func testANetworkWithNoCapNamesNoFigure() {
        let uncapped = WarrenNetworkInfo(
            environment: "production", degraded: false, defaultRateBps: nil, paymentsEnabled: true)
        XCTAssertNil(uncapped.capMbps)

        let zero = WarrenNetworkInfo(
            environment: "beta", degraded: true, defaultRateBps: 0, paymentsEnabled: false)
        XCTAssertNil(zero.capMbps)
    }

    /// The field is optional on the wire, so its absence must decode as "no
    /// cap" rather than failing the whole descriptor.
    func testAMissingCapStillYieldsADescriptor() throws {
        let info = try XCTUnwrap(
            WarrenNetworkInfoClient.info(
                fromEnvelope: #"{"ok":true,"environment":"production","degraded":false,"payments_enabled":true}"#
            ))

        XCTAssertNil(info.defaultRateBps)
        XCTAssertFalse(info.degraded)
        XCTAssertTrue(info.paymentsEnabled)
    }
}
