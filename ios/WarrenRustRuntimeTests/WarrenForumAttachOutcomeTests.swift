import XCTest

@testable import WarrenRustRuntime

/// The FFI envelopes of `warren_forum_attach_logs` and `warren_forum_code_probe`
/// are fixed JSON shapes single-sourced in the Rust `warren-forum` crate
/// (`attach_envelope`, `code_placement_envelope`). The attach table is
/// replayed from `fixtures/client-rules/forum_outcomes.json`, the file the
/// crate and the JVM decoder read on their side, so a class the crate adds
/// cannot silently fall into the generic failure.
final class WarrenForumAttachOutcomeTests: XCTestCase {
    private func expected(_ expect: [String: Any]) throws -> WarrenForumAttachOutcome {
        try outcome(kind: try ClientRulesFixtures.string(expect, "kind"), reason: expect["reason"] as? String)
    }

    private func outcome(kind: String, reason: String?) throws -> WarrenForumAttachOutcome {
        switch kind {
        case "attached": return .attached
        case "not-author": return .notAuthor
        case "expired": return .expired
        case "too-large": return .tooLarge
        case "clock-skew": return .clockSkew
        case "server-error": return .serverError
        case "failed": return .failed(reason: try XCTUnwrap(reason))
        case let other:
            XCTFail("unknown kind \(other)")
            return .failed(reason: other)
        }
    }

    func testEveryAttachCaseDecodesItsEnvelopeAsTheFixtureSays() throws {
        let fixture = try ClientRulesFixtures.load("forum_outcomes.json")
        let attach = try ClientRulesFixtures.object(fixture, "attach")
        let cases = try ClientRulesFixtures.cases(attach, "cases").filter { !ClientRulesFixtures.skippedOnIOS($0) }
        XCTAssertGreaterThanOrEqual(cases.count, 8, "only \(cases.count) attach cases reached this reader")
        for testCase in cases {
            let name = try ClientRulesFixtures.string(testCase, "name")
            let envelope = try ClientRulesFixtures.string(testCase, "envelope")
            let expect = try ClientRulesFixtures.object(testCase, "expect")
            XCTAssertEqual(
                WarrenAccountClient.forumAttachOutcome(fromEnvelope: envelope), try expected(expect), name)
        }
    }

    func testTheClientSideFailuresCarryTheirClass() throws {
        let fixture = try ClientRulesFixtures.load("forum_outcomes.json")
        let attach = try ClientRulesFixtures.object(fixture, "attach")
        let failures = try ClientRulesFixtures.object(attach, "client_side_failures")
        let cases = try ClientRulesFixtures.cases(failures, "cases")
        XCTAssertFalse(cases.isEmpty)
        for testCase in cases {
            let envelope = try ClientRulesFixtures.string(testCase, "envelope")
            let reason = try ClientRulesFixtures.string(testCase, "reason")
            XCTAssertEqual(WarrenAccountClient.forumAttachOutcome(fromEnvelope: envelope), .failed(reason: reason))
        }
    }

    func testTheTerminalKindsAreTheFixtures() throws {
        // Not the author, a dead session and a report over the cap end the
        // prompt; a clock fix, a settled tunnel or a recovered provider are
        // retries worth offering, as on Android.
        let fixture = try ClientRulesFixtures.load("forum_outcomes.json")
        let attach = try ClientRulesFixtures.object(fixture, "attach")
        let terminal = Set(try XCTUnwrap(attach["terminal_kinds"] as? [String]))
        let kinds = try XCTUnwrap(attach["_kinds"] as? [String])
        XCTAssertFalse(terminal.isEmpty)
        for kind in kinds {
            XCTAssertEqual(
                try outcome(kind: kind, reason: "transport").isTerminal, terminal.contains(kind), kind)
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

    func testTheJournalClassNamesTheOutcomeAndNeverAValue() {
        XCTAssertEqual(WarrenForumAttachOutcome.attached.journalClass, "attached")
        XCTAssertEqual(WarrenForumAttachOutcome.notAuthor.journalClass, "not-author")
        XCTAssertEqual(WarrenForumAttachOutcome.expired.journalClass, "expired")
        XCTAssertEqual(WarrenForumAttachOutcome.tooLarge.journalClass, "too-large")
        XCTAssertEqual(WarrenForumAttachOutcome.clockSkew.journalClass, "clock-skew")
        XCTAssertEqual(WarrenForumAttachOutcome.serverError.journalClass, "server-error")
        XCTAssertEqual(WarrenForumAttachOutcome.failed(reason: "http-502").journalClass, "http-502")
    }

    func testATypedCodeIsPlacedWithTheTopicItsMetaNames() {
        // `code_placement_envelope`: an attach session comes with the topic
        // the broker's meta named for it (0 for a pre-topic session). An
        // attach kind without a usable topic is not offered as one: the
        // upload could only ever be refused as a dead session.
        XCTAssertEqual(
            WarrenAccountClient.forumCodePlacement(fromEnvelope: #"{"kind":"attach","topic_id":4242}"#),
            .attach(topicId: 4242))
        XCTAssertEqual(
            WarrenAccountClient.forumCodePlacement(fromEnvelope: #"{"kind":"attach","topic_id":0}"#),
            .attach(topicId: 0))
        XCTAssertEqual(
            WarrenAccountClient.forumCodePlacement(fromEnvelope: #"{"kind":"attach"}"#), .attachWithoutTopic)
        XCTAssertEqual(
            WarrenAccountClient.forumCodePlacement(fromEnvelope: #"{"kind":"attach","topic_id":-1}"#),
            .attachWithoutTopic)
        XCTAssertEqual(
            WarrenAccountClient.forumCodePlacement(fromEnvelope: #"{"kind":"attach","topic_id":9007199254740993}"#),
            .attachWithoutTopic)
        XCTAssertEqual(WarrenAccountClient.forumCodePlacement(fromEnvelope: #"{"kind":"login"}"#), .login)
        XCTAssertEqual(WarrenAccountClient.forumCodePlacement(fromEnvelope: #"{"kind":"gone"}"#), .gone)
        XCTAssertEqual(WarrenAccountClient.forumCodePlacement(fromEnvelope: #"{"kind":"unknown"}"#), .unknown)
        XCTAssertEqual(WarrenAccountClient.forumCodePlacement(fromEnvelope: #"{"kind":"something"}"#), .unknown)
        XCTAssertEqual(WarrenAccountClient.forumCodePlacement(fromEnvelope: "nope"), .unknown)
        XCTAssertEqual(WarrenAccountClient.forumCodePlacement(fromEnvelope: nil), .unknown)
    }

    func testAPlacementNamesItsJournalClass() {
        XCTAssertEqual(WarrenForumCodePlacement.attach(topicId: 42).journalClass, "attach")
        XCTAssertEqual(WarrenForumCodePlacement.attachWithoutTopic.journalClass, "attach-no-topic")
        XCTAssertEqual(WarrenForumCodePlacement.login.journalClass, "login")
        XCTAssertEqual(WarrenForumCodePlacement.gone.journalClass, "gone")
        XCTAssertEqual(WarrenForumCodePlacement.unknown.journalClass, "unknown")
    }
}
