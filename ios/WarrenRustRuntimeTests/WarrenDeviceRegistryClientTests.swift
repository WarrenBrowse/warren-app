//
//  WarrenDeviceRegistryClientTests.swift
//  WarrenRustRuntimeTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import XCTest

@testable import WarrenRustRuntime

/// What Swift makes of the envelopes `warren_standing::device_registry`
/// answers for the list, the rename and the revocation. Pure decoding, off
/// any device and any network.
final class WarrenDeviceRegistryClientTests: XCTestCase {
    func testAListCarriesEveryDeviceWithThisOneMarkedAndTheOlderClientDay() throws {
        let envelope = """
            {"ok":true,"devices":[
            {"device_id":"AAAAAAAAAAAAAAAAAAAAAA","name":"brave otter","platform":"ios",
             "app_version":"1.1.40","registered_on":"2026-10-01","last_seen_on":"2026-10-28","this_device":true},
            {"device_id":"BBBBBBBBBBBBBBBBBBBBBB","name":"quiet heron","platform":"windows",
             "app_version":"1.1.39","registered_on":"2026-09-01","last_seen_on":"2026-10-26","this_device":false}
            ],"older_client_seen_on":"2026-10-25"}
            """

        let list = try WarrenDeviceRegistryClient.list(fromEnvelope: envelope).get()

        XCTAssertEqual(
            list,
            WarrenAccountDeviceList(
                devices: [
                    WarrenAccountDevice(
                        id: "AAAAAAAAAAAAAAAAAAAAAA", name: "brave otter", platform: "ios",
                        appVersion: "1.1.40", lastSeenOn: WarrenRegistryDay("2026-10-28"), thisDevice: true
                    ),
                    WarrenAccountDevice(
                        id: "BBBBBBBBBBBBBBBBBBBBBB", name: "quiet heron", platform: "windows",
                        appVersion: "1.1.39", lastSeenOn: WarrenRegistryDay("2026-10-26"), thisDevice: false
                    ),
                ],
                olderClientSeenOn: WarrenRegistryDay("2026-10-25")
            )
        )
    }

    func testAListWithoutAnOlderClientHasNoOlderClientDay() throws {
        let list = try WarrenDeviceRegistryClient.list(
            fromEnvelope: #"{"ok":true,"devices":[],"older_client_seen_on":null}"#
        ).get()

        XCTAssertEqual(list, WarrenAccountDeviceList(devices: [], olderClientSeenOn: nil))
    }

    /// Nothing could disconnect or rename a device without its id.
    func testARowWithoutAnIdIsDropped() throws {
        let list = try WarrenDeviceRegistryClient.list(
            fromEnvelope: #"{"ok":true,"devices":[{"name":"ghost","platform":"linux"}]}"#
        ).get()

        XCTAssertEqual(list.devices, [])
    }

    func testEveryErrorCodeTheCrateNamesIsUnderstood() {
        for (code, expected) in [
            ("no_wallet", WarrenDeviceRegistryError.noWallet),
            ("invalid_device", .invalidDevice),
            ("invalid_name", .invalidName),
            ("not_found", .notFound),
            ("revoked", .revoked),
            ("unavailable", .unavailable),
        ] {
            let envelope = #"{"ok":false,"error":"\#(code)"}"#
            XCTAssertEqual(WarrenDeviceRegistryClient.list(fromEnvelope: envelope), .failure(expected), code)
            XCTAssertEqual(WarrenDeviceRegistryClient.action(fromEnvelope: envelope), .failure(expected), code)
        }
    }

    /// A code a newer crate adds, or an envelope nobody can read, must never
    /// read as a call that happened.
    func testAnUnknownCodeOrAnUnreadableEnvelopeIsUnavailable() {
        for envelope in [nil, "", "not json", #"{"ok":false,"error":"surprise"}"#, #"{"devices":[]}"#] {
            XCTAssertEqual(
                WarrenDeviceRegistryClient.list(fromEnvelope: envelope), .failure(.unavailable), envelope ?? "nil")
            XCTAssertEqual(
                WarrenDeviceRegistryClient.action(fromEnvelope: envelope), .failure(.unavailable), envelope ?? "nil")
        }
    }

    func testAnActionSaysWhetherItConcernedThisDevice() {
        XCTAssertEqual(
            WarrenDeviceRegistryClient.action(fromEnvelope: #"{"ok":true,"this_device":true}"#), .success(true))
        XCTAssertEqual(
            WarrenDeviceRegistryClient.action(fromEnvelope: #"{"ok":true,"this_device":false}"#), .success(false))
    }

    /// The FFI reads 32 bytes from the pointer: anything shorter never crosses.
    func testASeedOfTheWrongLengthNeverReachesTheRegistry() {
        XCTAssertEqual(WarrenDeviceRegistryClient.list(seed: Data(count: 31)), .failure(.noWallet))
        XCTAssertEqual(WarrenDeviceRegistryClient.revoke(seed: Data(), id: "x"), .failure(.noWallet))
    }

    func testARegistryDayIsReadOnlyInTheRegistrySpellingAndOrdersByDate() {
        XCTAssertNil(WarrenRegistryDay("2026-10-28T00:00:00Z"))
        XCTAssertNil(WarrenRegistryDay("2026-13-01"))
        XCTAssertNil(WarrenRegistryDay("28/10/2026"))
        XCTAssertLessThan(WarrenRegistryDay("2026-09-30")!, WarrenRegistryDay("2026-10-01")!)
        XCTAssertEqual(
            WarrenRegistryDay(utc: Date(timeIntervalSince1970: 1_793_231_999)),  // 2026-10-28T23:59:59Z
            WarrenRegistryDay("2026-10-28")
        )
    }
}
