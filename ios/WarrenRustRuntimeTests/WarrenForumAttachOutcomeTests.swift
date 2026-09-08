import XCTest

@testable import WarrenRustRuntime

/// The FFI envelopes of `warren_forum_attach_logs` and `warren_forum_code_probe`
/// are fixed JSON shapes single-sourced in the Rust `warren-forum` crate
/// (`attach_envelope`, `code_probe_envelope`), the same ones the Android
/// decoder reads. This pins the Swift decoders to them off-device, so a class
/// the crate adds cannot silently fall into the generic failure.
final class WarrenForumAttachOutcomeTests: XCTestCase {
    func testEveryAttachClassDecodesToItsOutcome() {
        let table: [(String, WarrenForumAttachOutcome)] = [
            (#"{"ok":true}"#, .attached),
            (#"{"ok":false,"error":"not-author"}"#, .notAuthor),
            (#"{"ok":false,"error":"expired"}"#, .expired),
            (#"{"ok":false,"error":"too-large"}"#, .tooLarge),
            (#"{"ok":false,"error":"clock-skew"}"#, .clockSkew),
            (#"{"ok":false,"error":"server-error"}"#, .serverError),
            (#"{"ok":false,"error":"error","reason":"transport"}"#, .failed(reason: "transport")),
            (#"{"ok":false,"error":"error","reason":"upload-timeout"}"#, .failed(reason: "upload-timeout")),
            (#"{"ok":false,"error":"error","reason":"http-418"}"#, .failed(reason: "http-418")),
        ]
        for (envelope, expected) in table {
            XCTAssertEqual(WarrenAccountClient.forumAttachOutcome(fromEnvelope: envelope), expected, envelope)
        }
    }

    func testAnUnreadableOrUnnamedEnvelopeIsAFailureWithAClass() {
        XCTAssertEqual(WarrenAccountClient.forumAttachOutcome(fromEnvelope: nil), .failed(reason: "unknown"))
        XCTAssertEqual(WarrenAccountClient.forumAttachOutcome(fromEnvelope: "not json"), .failed(reason: "unknown"))
        XCTAssertEqual(
            WarrenAccountClient.forumAttachOutcome(fromEnvelope: #"{"ok":false,"error":"error"}"#),
            .failed(reason: "unknown"))
        XCTAssertEqual(
            WarrenAccountClient.forumAttachOutcome(fromEnvelope: #"{"ok":false,"error":"something-new"}"#),
            .failed(reason: "unknown"))
    }

    func testOnlyTheOutcomesNoRetryCanChangeAreTerminal() {
        // Not the author, a dead session and a report over the cap end the
        // prompt; a clock fix, a settled tunnel or a recovered provider are
        // retries worth offering, as on Android.
        XCTAssertTrue(WarrenForumAttachOutcome.notAuthor.isTerminal)
        XCTAssertTrue(WarrenForumAttachOutcome.expired.isTerminal)
        XCTAssertTrue(WarrenForumAttachOutcome.tooLarge.isTerminal)
        XCTAssertFalse(WarrenForumAttachOutcome.attached.isTerminal)
        XCTAssertFalse(WarrenForumAttachOutcome.clockSkew.isTerminal)
        XCTAssertFalse(WarrenForumAttachOutcome.serverError.isTerminal)
        XCTAssertFalse(WarrenForumAttachOutcome.failed(reason: "transport").isTerminal)
    }

    func testTheJournalClassNamesTheOutcomeAndNeverAValue() {
        XCTAssertEqual(WarrenForumAttachOutcome.attached.journalClass, "attached")
        XCTAssertEqual(WarrenForumAttachOutcome.notAuthor.journalClass, "not-author")
        XCTAssertEqual(WarrenForumAttachOutcome.expired.journalClass, "expired")
        XCTAssertEqual(WarrenForumAttachOutcome.tooLarge.journalClass, "too-large")
        XCTAssertEqual(WarrenForumAttachOutcome.clockSkew.journalClass, "clock-skew")
        XCTAssertEqual(WarrenForumAttachOutcome.serverError.journalClass, "server-error")
        XCTAssertEqual(WarrenForumAttachOutcome.failed(reason: "http-502").journalClass, "http-502")
    }

    func testTheCodeProbeKindsDecodeAndAnythingElseIsUnknown() {
        XCTAssertEqual(WarrenAccountClient.forumCodeKind(fromEnvelope: #"{"kind":"login"}"#), .login)
        XCTAssertEqual(WarrenAccountClient.forumCodeKind(fromEnvelope: #"{"kind":"attach"}"#), .attach)
        XCTAssertEqual(WarrenAccountClient.forumCodeKind(fromEnvelope: #"{"kind":"gone"}"#), .gone)
        XCTAssertEqual(WarrenAccountClient.forumCodeKind(fromEnvelope: #"{"kind":"unknown"}"#), .unknown)
        XCTAssertEqual(WarrenAccountClient.forumCodeKind(fromEnvelope: #"{"kind":"something"}"#), .unknown)
        XCTAssertEqual(WarrenAccountClient.forumCodeKind(fromEnvelope: "nope"), .unknown)
        XCTAssertEqual(WarrenAccountClient.forumCodeKind(fromEnvelope: nil), .unknown)
    }
}
