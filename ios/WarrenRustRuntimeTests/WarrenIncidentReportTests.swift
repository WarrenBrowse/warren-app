//
//  WarrenIncidentReportTests.swift
//  WarrenRustRuntimeTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import XCTest

@testable import WarrenRustRuntime

/// How the shared incident envelope reads.
///
/// The exit key-change alert's "Report to Warren" button opened a static FAQ
/// page, so the report the button names never left the device and the operator
/// feed never learned of a mismatch a user had seen. The envelope is now the
/// same one Android answers with, single-sourced in the `warren-incidents`
/// crate, and its shapes are pinned by `fixtures/client-rules/incident_reports.json`.
final class WarrenIncidentReportTests: XCTestCase {
    private func outcome(_ envelope: String?) -> WarrenIncidentOutcome {
        WarrenIncidentReport.outcome(fromEnvelope: envelope)
    }

    func testAnAcceptedReportReadsAsSent() {
        XCTAssertEqual(outcome(#"{"ok":true}"#), .sent)
        XCTAssertTrue(outcome(#"{"ok":true}"#).didSend)
    }

    func testARefusedReportCarriesTheClassItWasRefusedUnder() {
        XCTAssertEqual(outcome(#"{"ok":false,"reason":"budget"}"#), .notSent(reason: "budget"))
        XCTAssertEqual(outcome(#"{"ok":false,"reason":"transport"}"#), .notSent(reason: "transport"))
        XCTAssertEqual(outcome(#"{"ok":false,"reason":"rejected"}"#), .notSent(reason: "rejected"))
    }

    /// An envelope that cannot be read must never pass as a send: a report the
    /// feed did not get, counted as delivered, is a silent gap.
    func testAnUnreadableEnvelopeIsNeverASend() {
        for envelope in [nil, "", "not json", "{}", #"{"ok":"true"}"#, #"{"ok":false}"#] {
            XCTAssertFalse(outcome(envelope).didSend, "\(envelope ?? "nil") read as sent")
        }
        XCTAssertEqual(outcome("{}"), .notSent(reason: "unknown"))
    }

    /// A seed of the wrong length never reaches the FFI, which would read 32
    /// bytes out of it regardless.
    func testASeedOfTheWrongLengthIsRefusedBeforeTheFfi() {
        for count in [0, 31, 33] {
            let result = WarrenIncidentReport.pubkeyMismatch(
                seed: Data(repeating: 0, count: count),
                exitIdHex: "00",
                oldPubkeyHex: "aa",
                newPubkeyHex: "bb",
                countryCode: "ch",
                city: ""
            )
            XCTAssertEqual(result, .notSent(reason: "identity"), "\(count) bytes")
        }
    }
}
