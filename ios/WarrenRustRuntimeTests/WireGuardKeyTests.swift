//
//  WireGuardKeyTests.swift
//  WarrenRustRuntimeTests
//
//  Created by Emils on 2026-04-07.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import XCTest

@testable import WarrenRustRuntime
@testable import WarrenTypes

// The legacy `WireGuardKitTypes` module was removed with the WireGuard
// backend. These tests now cover the Warren `WireGuard.*` x25519 key
// types directly (serialization round-trips, string init, validation).
typealias PrivateKey = WireGuard.PrivateKey
typealias PublicKey = WireGuard.PublicKey
typealias PreSharedKey = WireGuard.PreSharedKey

class WireGuardKeyTests: XCTestCase {

    // MARK: - Round-trip serialization

    func testPrivateKeyRoundTrip() throws {
        let key = PrivateKey()
        let encodedData = try JSONEncoder().encode(key)
        let decodedKey = try JSONDecoder().decode(PrivateKey.self, from: encodedData)

        XCTAssertEqual(key, decodedKey)
    }

    func testPublicKeyRoundTrip() throws {
        let key = PrivateKey().publicKey
        let encodedData = try JSONEncoder().encode(key)
        let decodedKey = try JSONDecoder().decode(PublicKey.self, from: encodedData)

        XCTAssertEqual(key, decodedKey)
    }

    func testPreSharedKeyRoundTrip() throws {
        let rawData = Data((0..<32).map { _ in UInt8.random(in: 0...255) })
        let key = PreSharedKey(rawValue: rawData)!
        let encodedData = try JSONEncoder().encode(key)
        let decodedKey = try JSONDecoder().decode(PreSharedKey.self, from: encodedData)

        XCTAssertEqual(key.rawValue, decodedKey.rawValue)
    }

    // MARK: - Public key derivation

    func testPublicKeyDerivationIsDeterministic() throws {
        let rawKeyData = Data((0..<32).map { _ in UInt8.random(in: 0...255) })

        let key1 = PrivateKey(rawValue: rawKeyData)!
        let key2 = PrivateKey(rawValue: rawKeyData)!

        XCTAssertEqual(key1.publicKey.rawValue, key2.publicKey.rawValue)
    }

    // MARK: - Init from string

    func testInitFromBase64() throws {
        let key = PrivateKey()
        let base64 = key.base64Key

        let restored = PrivateKey(base64Key: base64)
        XCTAssertNotNil(restored)
        XCTAssertEqual(key, restored)
    }

    func testInitFromHex() throws {
        let key = PrivateKey()
        let hex = key.hexKey

        let restored = PrivateKey(hexKey: hex)
        XCTAssertNotNil(restored)
        XCTAssertEqual(key, restored)
    }

    // MARK: - Validation

    func testRejectsInvalidKeyLength() {
        let tooShort = Data(repeating: 0, count: 16)
        let tooLong = Data(repeating: 0, count: 64)

        XCTAssertNil(PrivateKey(rawValue: tooShort))
        XCTAssertNil(PrivateKey(rawValue: tooLong))
        XCTAssertNil(PublicKey(rawValue: tooShort))
        XCTAssertNil(PublicKey(rawValue: tooLong))
        XCTAssertNil(PreSharedKey(rawValue: tooShort))
        XCTAssertNil(PreSharedKey(rawValue: tooLong))
    }

    func testDecodingInvalidDataFails() {
        let invalidJSON = try! JSONEncoder().encode(Data(repeating: 0, count: 16))

        XCTAssertThrowsError(try JSONDecoder().decode(PrivateKey.self, from: invalidJSON))
        XCTAssertThrowsError(try JSONDecoder().decode(PublicKey.self, from: invalidJSON))
        XCTAssertThrowsError(try JSONDecoder().decode(PreSharedKey.self, from: invalidJSON))
    }
}
