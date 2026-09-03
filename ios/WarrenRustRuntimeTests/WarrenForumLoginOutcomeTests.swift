import XCTest

@testable import WarrenRustRuntime

/// The FFI envelope for `warren_forum_login` is a fixed set of JSON shapes
/// single-sourced in the Rust `warren-forum` crate and pinned, outcome by
/// outcome, in `fixtures/client-rules/forum_outcomes.json`. This replays the
/// login table through the Swift decoder, off-device, so a new envelope kind
/// cannot silently fall into the generic failure the way `clock-skew` did on
/// 2026-08-18.
final class WarrenForumLoginOutcomeTests: XCTestCase {
    private func expected(_ expect: [String: Any]) throws -> WarrenForumLoginOutcome {
        switch try ClientRulesFixtures.string(expect, "kind") {
        case "approved":
            guard let handle = expect["handle"] as? String else { return .approved(nil) }
            let slot = (expect["notify_slot"] as? NSNumber).map { UInt32(truncating: $0) }
            return .approved(WarrenForumIdentity(handle: handle, notifySlot: slot))
        case "subscription-required":
            return .subscriptionRequired
        case "clock-skew":
            return .clockSkew
        case "expired":
            return .expired
        case "failed":
            return .failed(reason: try ClientRulesFixtures.string(expect, "reason"))
        case let other:
            XCTFail("unknown kind \(other)")
            return .failed(reason: other)
        }
    }

    func testEveryLoginCaseDecodesItsEnvelopeAsTheFixtureSays() throws {
        let fixture = try ClientRulesFixtures.load("forum_outcomes.json")
        let login = try ClientRulesFixtures.object(fixture, "login")
        let cases = try ClientRulesFixtures.cases(login, "cases").filter { !ClientRulesFixtures.skippedOnIOS($0) }
        XCTAssertGreaterThanOrEqual(cases.count, 15, "only \(cases.count) login cases reached this reader")
        for testCase in cases {
            let name = try ClientRulesFixtures.string(testCase, "name")
            let envelope = try ClientRulesFixtures.string(testCase, "envelope")
            let expect = try ClientRulesFixtures.object(testCase, "expect")
            XCTAssertEqual(
                WarrenAccountClient.forumLoginOutcome(fromEnvelope: envelope), try expected(expect), name)
        }
    }

    func testTheClientSideFailuresCarryTheirClass() throws {
        let fixture = try ClientRulesFixtures.load("forum_outcomes.json")
        let login = try ClientRulesFixtures.object(fixture, "login")
        let failures = try ClientRulesFixtures.object(login, "client_side_failures")
        for testCase in try ClientRulesFixtures.cases(failures, "cases") {
            let envelope = try ClientRulesFixtures.string(testCase, "envelope")
            let reason = try ClientRulesFixtures.string(testCase, "reason")
            XCTAssertEqual(WarrenAccountClient.forumLoginOutcome(fromEnvelope: envelope), .failed(reason: reason))
        }
    }

    func testTheTerminalKindsAreTheFixtures() throws {
        let fixture = try ClientRulesFixtures.load("forum_outcomes.json")
        let login = try ClientRulesFixtures.object(fixture, "login")
        let terminal = Set(try XCTUnwrap(login["terminal_kinds"] as? [String]))
        let outcomes: [(String, WarrenForumLoginOutcome)] = [
            ("approved", .approved(nil)),
            ("subscription-required", .subscriptionRequired),
            ("clock-skew", .clockSkew),
            ("expired", .expired),
            ("failed", .failed(reason: "transport")),
        ]
        XCTAssertEqual(Set(outcomes.map(\.0)), Set(try XCTUnwrap(login["_kinds"] as? [String])))
        for (kind, outcome) in outcomes {
            XCTAssertEqual(outcome.isTerminal, terminal.contains(kind), kind)
        }
    }

    func testAnUnreadableEnvelopeIsAGenericFailure() {
        XCTAssertEqual(WarrenAccountClient.forumLoginOutcome(fromEnvelope: "not json"), .failed(reason: "unknown"))
        XCTAssertEqual(WarrenAccountClient.forumLoginOutcome(fromEnvelope: nil), .failed(reason: "unknown"))
        XCTAssertEqual(
            WarrenAccountClient.forumLoginOutcome(fromEnvelope: #"{"ok":false,"error":"error"}"#),
            .failed(reason: "unknown"))
    }
}
