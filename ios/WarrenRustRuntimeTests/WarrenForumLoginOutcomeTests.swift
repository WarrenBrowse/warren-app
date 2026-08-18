import XCTest

@testable import WarrenRustRuntime

/// The FFI envelope for `warren_forum_login` is a fixed set of JSON shapes
/// single-sourced in the Rust `warren-forum` crate; this pins the Swift side
/// of that contract, off-device, so a new envelope error cannot silently fall
/// into the generic failure the way `clock-skew` did on 2026-08-18.
final class WarrenForumLoginOutcomeTests: XCTestCase {
    func testOkMapsToApproved() {
        XCTAssertEqual(WarrenAccountClient.forumLoginOutcome(fromEnvelope: #"{"ok":true}"#), .approved)
    }

    func testSubscriptionRequiredIsRecognised() {
        XCTAssertEqual(
            WarrenAccountClient.forumLoginOutcome(fromEnvelope: #"{"ok":false,"error":"subscription-required"}"#),
            .subscriptionRequired
        )
    }

    func testClockSkewIsRecognised() {
        // The one failure the user repairs themselves; it must not collapse
        // into "try again in a moment".
        XCTAssertEqual(
            WarrenAccountClient.forumLoginOutcome(fromEnvelope: #"{"ok":false,"error":"clock-skew"}"#),
            .clockSkew
        )
    }

    func testAnyOtherErrorIsAGenericFailure() {
        XCTAssertEqual(
            WarrenAccountClient.forumLoginOutcome(fromEnvelope: #"{"ok":false,"error":"error"}"#),
            .failed
        )
        XCTAssertEqual(WarrenAccountClient.forumLoginOutcome(fromEnvelope: "not json"), .failed)
        XCTAssertEqual(WarrenAccountClient.forumLoginOutcome(fromEnvelope: nil), .failed)
    }
}
