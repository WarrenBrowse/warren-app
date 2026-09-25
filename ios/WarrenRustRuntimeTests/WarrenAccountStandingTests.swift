//
//  WarrenAccountStandingTests.swift
//  WarrenRustRuntimeTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import Foundation
import XCTest

@testable import WarrenRustRuntime

final class WarrenAccountStandingTests: XCTestCase {
    private let strikeJSON =
        #"{"day_unix_secs":1790208000,"category":"copyright","exit_country":"FI","port":51413,"case_reference":"PF-1"}"#

    func testTheEnvelopeReadsTheStrikesAndTheStrikesToWarnAbout() throws {
        let envelope =
            #"{"ok":true,"reported":true,"standing":{"strikes":[\#(strikeJSON)],"threshold":3,"window_days":90,"ban":null},"new_strikes":[{"strike":\#(strikeJSON),"ordinal":1,"threshold":3}]}"#

        let poll = try XCTUnwrap(WarrenStandingPoll.parse(envelope: envelope))

        XCTAssertTrue(poll.ok)
        XCTAssertTrue(poll.reported)
        let strike = try XCTUnwrap(poll.standing?.strikes.first)
        XCTAssertEqual(strike.port, 51413)
        XCTAssertEqual(strike.caseReference, "PF-1")
        XCTAssertEqual(strike.day, Date(timeIntervalSince1970: 1_790_208_000))
        XCTAssertEqual(poll.newStrikes, [WarrenStrikeNotice(strike: strike, ordinal: 1, threshold: 3)])
        XCTAssertEqual(poll.standing?.latestStrike, WarrenStrikeNotice(strike: strike, ordinal: 1, threshold: 3))
    }

    func testABanCarriesWhatItIsForAndWhenItLapses() throws {
        let envelope =
            #"{"ok":true,"reported":true,"standing":{"strikes":[],"threshold":3,"window_days":90,"ban":{"reason":"port_forwarding_abuse","banned_at_unix_secs":1790000000,"lapses_at_unix_secs":1821744000,"in_force":true,"block_reason":"[BANNED_PORT_FORWARDING] x"}},"new_strikes":[]}"#

        let ban = try XCTUnwrap(WarrenStandingPoll.parse(envelope: envelope)?.standing?.ban)

        XCTAssertEqual(
            ban,
            WarrenAccountBan(
                portForwarding: true,
                lapsesAt: Date(timeIntervalSince1970: 1_821_744_000),
                inForce: true
            )
        )
    }

    func testAnApiWithoutTheStandingIsNotReportedAndNotAFailure() throws {
        let poll = try XCTUnwrap(
            WarrenStandingPoll.parse(envelope: #"{"ok":true,"reported":false,"standing":null,"new_strikes":[]}"#)
        )

        XCTAssertTrue(poll.ok)
        XCTAssertFalse(poll.reported)
        XCTAssertNil(poll.standing)
    }

    func testAnUnreadableEnvelopeIsNoPoll() {
        XCTAssertNil(WarrenStandingPoll.parse(envelope: "not json"))
        XCTAssertNil(WarrenStandingPoll.parse(envelope: nil))
    }

    func testAStrikeRendersNeitherItsCaseNorItsPort() {
        let strike = WarrenAccountStrike(
            day: Date(timeIntervalSince1970: 0),
            category: "copyright",
            exitCountry: nil,
            port: 51413,
            caseReference: "PF-2026-0042"
        )

        XCTAssertFalse(strike.description.contains("PF-2026-0042"))
        XCTAssertFalse(strike.description.contains("51413"))
        XCTAssertFalse(strike.dismissalKey.contains("PF-2026-0042"))
    }

    func testTheDismissalKeyTellsStrikesApartAndIsStable() {
        func key(_ reference: String) -> String {
            WarrenAccountStrike(
                day: Date(timeIntervalSince1970: 0), category: "spam", exitCountry: nil, port: 1,
                caseReference: reference
            ).dismissalKey
        }

        XCTAssertEqual(key("PF-1"), key("PF-1"))
        XCTAssertNotEqual(key("PF-1"), key("PF-2"))
    }

    func testTheBanTokenOfAnAuthFailedReasonIsRead() {
        XCTAssertEqual(
            WarrenBanReason.of(authFailedReason: "[BANNED_PORT_FORWARDING] the API refused"),
            .portForwarding
        )
        XCTAssertEqual(WarrenBanReason.of(authFailedReason: "[BANNED] exit rejected"), .other)
        XCTAssertNil(WarrenBanReason.of(authFailedReason: "subscription expired"))
    }
}
