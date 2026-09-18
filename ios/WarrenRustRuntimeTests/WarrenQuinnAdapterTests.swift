//
//  WarrenQuinnAdapterTests.swift
//  WarrenRustRuntimeTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import Foundation
import XCTest

@testable import WarrenRustRuntime

/// The pure seams of `WarrenQuinnAdapter`, the Swift side of the tunnel FFI.
///
/// The adapter is the whole datapath plumbing between the app and the Rust
/// engine, and it appeared in the test corpus only as a mock: every test that
/// named it substituted it. The parts below need no tunnel and no network, and
/// each one decides something a user sees or a security decision rests on.
final class WarrenQuinnAdapterTests: XCTestCase {
    // MARK: - Key length

    /// Both documented error paths of `WarrenQuinnAdapterError`. A key of the
    /// wrong length reaching the FFI is a buffer the Rust side reads 32 bytes
    /// out of regardless, so the refusal has to happen here.
    func testAKeyOfTheWrongLengthIsRefusedRatherThanPassedOn() {
        for count in [0, 31, 33, 64] {
            XCTAssertThrowsError(
                try WarrenQuinnAdapter.fixedKeyBytes(
                    Data(repeating: 0xab, count: count), count: 32, field: "exitPubkey"),
                "a \(count)-byte key was accepted"
            ) { error in
                guard case let WarrenQuinnAdapterError.invalidKeyLength(field, expected, actual) = error
                else {
                    return XCTFail("unexpected error \(error)")
                }
                XCTAssertEqual(field, "exitPubkey")
                XCTAssertEqual(expected, 32)
                XCTAssertEqual(actual, count)
            }
        }
    }

    func testAKeyOfTheRightLengthComesBackAsItsBytes() throws {
        let key = Data((0..<32).map { UInt8($0) })
        let bytes = try WarrenQuinnAdapter.fixedKeyBytes(key, count: 32, field: "exitPubkey")
        XCTAssertEqual(bytes, [UInt8](key))
    }

    // MARK: - Pin mismatch decoding

    /// The exact payload `warren_tunnel_ffi.rs` writes when an exit serves a
    /// key other than the pinned one. Its keys are snake_case and the Swift
    /// struct's are not, so the mapping is the contract: decode it wrong and
    /// the user is asked to judge an empty dialog.
    func testTheMismatchTheEngineWritesDecodesFieldForField() throws {
        let json = """
            {"exit_id":"0011223344556677","observed":"\(String(repeating: "b", count: 64))",\
            "pinned":"\(String(repeating: "a", count: 64))","country":"ch"}
            """
        let mismatch = try XCTUnwrap(WarrenQuinnAdapter.decodePinMismatch(json))
        XCTAssertEqual(mismatch.exitId, "0011223344556677")
        XCTAssertEqual(mismatch.observed, String(repeating: "b", count: 64))
        XCTAssertEqual(mismatch.pinned, String(repeating: "a", count: 64))
        XCTAssertEqual(mismatch.country, "ch")
    }

    /// An exit whose country the directory did not carry still has to produce
    /// a decodable mismatch: failing to decode would swallow the warning.
    func testAMismatchWithoutACountryStillDecodes() throws {
        let json = """
            {"exit_id":"00","observed":"bb","pinned":"aa","country":""}
            """
        let mismatch = try XCTUnwrap(WarrenQuinnAdapter.decodePinMismatch(json))
        XCTAssertEqual(mismatch.country, "")
    }

    func testAnythingThatIsNotAMismatchDecodesToNothing() {
        for json in ["", "null", "{}", "not json", #"{"exit_id":"00"}"#] {
            XCTAssertNil(
                WarrenQuinnAdapter.decodePinMismatch(json), "\(json.debugDescription) decoded")
        }
    }

    // MARK: - Address rendering

    /// The address the connection panel shows the user comes through here.
    func testAnAssignedAddressRendersDotted() {
        XCTAssertEqual(dottedIPv4((0, 0, 0, 0)), "0.0.0.0")
        XCTAssertEqual(dottedIPv4((255, 255, 255, 255)), "255.255.255.255")
        XCTAssertEqual(dottedIPv4((10, 64, 0, 7)), "10.64.0.7")
    }

    // MARK: - Fixed-width marshalling

    func testThirtyTwoBytesMarshalIntoTheTupleInOrder() {
        let bytes = (0..<32).map { UInt8($0) }
        let tuple = tupleFrom32(bytes)
        XCTAssertEqual(tuple.0, 0)
        XCTAssertEqual(tuple.1, 1)
        XCTAssertEqual(tuple.30, 30)
        XCTAssertEqual(tuple.31, 31)
    }

    /// A short array must not read past its end: the tuple is 32 bytes wide
    /// whatever it is given, and the tail is zero rather than whatever was
    /// next in memory.
    func testAShortArrayMarshalsWithAZeroTailRatherThanReadingPastIt() {
        let tuple = tupleFrom32([1, 2, 3])
        XCTAssertEqual(tuple.0, 1)
        XCTAssertEqual(tuple.1, 2)
        XCTAssertEqual(tuple.2, 3)
        XCTAssertEqual(tuple.3, 0)
        XCTAssertEqual(tuple.31, 0)
    }
}
