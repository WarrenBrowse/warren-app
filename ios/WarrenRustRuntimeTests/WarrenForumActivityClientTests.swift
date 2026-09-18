//
//  WarrenForumActivityClientTests.swift
//  WarrenRustRuntimeTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import XCTest

@testable import WarrenRustRuntime

/// What Swift makes of the envelopes the shared crate hands back for the
/// digest fetch, the panel read and the mark-seen write. Pure decoding, so it
/// runs off any device and off any network.
final class WarrenForumActivityClientTests: XCTestCase {
    // MARK: - Digest

    func testAFreshDocumentIsHandedOverWithItsFetchClass() {
        let digest = WarrenForumActivityClient.digest(
            fromEnvelope: #"{"counts":"03f","fetch":"ok"}"#)
        XCTAssertEqual(digest, WarrenForumDigest(counts: "03f", fetch: .ok))
    }

    func testEveryFetchClassTheCrateNamesIsUnderstood() {
        for (token, expected) in [
            ("ok", WarrenForumDigestFetch.ok),
            ("not-modified", .notModified),
            ("rejected", .rejected),
            ("transport", .transport),
        ] {
            let digest = WarrenForumActivityClient.digest(
                fromEnvelope: #"{"counts":null,"fetch":"\#(token)"}"#)
            XCTAssertEqual(digest.fetch, expected, token)
            XCTAssertNil(digest.counts)
        }
    }

    /// An envelope nobody can read must not read as a server that answered:
    /// only the unreachable class retries soon, and only it holds no document.
    func testAnUnreadableEnvelopeIsAFetchThatNeverReachedTheServer() {
        for envelope in [nil, "", "not json", #"{"fetch":"surprise"}"#] {
            let digest = WarrenForumActivityClient.digest(fromEnvelope: envelope)
            XCTAssertEqual(digest.fetch, .transport, envelope ?? "nil")
            XCTAssertNil(digest.counts)
            XCTAssertTrue(digest.fetch.isUnreachable)
        }
    }

    func testOnlyAFetchThatNeverReachedTheServerEarnsTheFastRetry() {
        XCTAssertTrue(WarrenForumDigestFetch.transport.isUnreachable)
        for fetch: WarrenForumDigestFetch in [.ok, .notModified, .rejected] {
            XCTAssertFalse(fetch.isUnreachable, fetch.rawValue)
        }
    }

    // MARK: - Panel

    func testAPanelReadCarriesItsRowsWithEveryFieldTheCrateValidated() throws {
        let envelope = """
            {"ok":true,"notifications":[
              {"id":42,"kind":"replied","unread":true,"created_at":1800000000,
               "title":"A topic","actor":"lusab-babad-dovok","excerpt":"a line","path":"/t/86/4"}
            ]}
            """
        guard case let .ok(rows) = WarrenForumActivityClient.notifications(fromEnvelope: envelope)
        else {
            return XCTFail("a well-formed panel read must carry its rows")
        }
        let row = try XCTUnwrap(rows.first)
        XCTAssertEqual(row.id, 42)
        XCTAssertEqual(row.kind, .replied)
        XCTAssertTrue(row.unread)
        XCTAssertEqual(row.createdAt, Date(timeIntervalSince1970: 1_800_000_000))
        XCTAssertEqual(row.title, "A topic")
        XCTAssertEqual(row.actor, "lusab-babad-dovok")
        XCTAssertEqual(row.excerpt, "a line")
        XCTAssertEqual(row.path, "/t/86/4")
    }

    /// A Discourse upgrade adding a notification type must not make the row
    /// vanish from the panel.
    func testAKindThisVersionHasNoWordingForIsStillARow() {
        let envelope = """
            {"ok":true,"notifications":[
              {"id":1,"kind":"invented_last_week","unread":false,"created_at":1}
            ]}
            """
        guard case let .ok(rows) = WarrenForumActivityClient.notifications(fromEnvelope: envelope)
        else {
            return XCTFail("an unknown kind must not empty the panel")
        }
        XCTAssertEqual(rows.map(\.kind), [.other])
    }

    func testARowWithoutTheTwoFieldsAPanelNeedsIsDroppedRatherThanRendered() {
        let envelope = """
            {"ok":true,"notifications":[
              {"kind":"liked","created_at":1},
              {"id":2,"kind":"liked"},
              {"id":3,"kind":"liked","created_at":2}
            ]}
            """
        guard case let .ok(rows) = WarrenForumActivityClient.notifications(fromEnvelope: envelope)
        else {
            return XCTFail("the readable row must survive its malformed neighbours")
        }
        XCTAssertEqual(rows.map(\.id), [3])
    }

    func testAFailedPanelReadCarriesItsClassAndNeverAValue() {
        XCTAssertEqual(
            WarrenForumActivityClient.notifications(
                fromEnvelope: #"{"ok":false,"error":"error","reason":"transport"}"#),
            .failed(reason: "transport"))
        XCTAssertEqual(
            WarrenForumActivityClient.notifications(fromEnvelope: "not json"),
            .failed(reason: "unknown"))
    }

    // MARK: - Mark seen

    func testOnlyAnExplicitOkIsAWriteTheProviderTook() {
        XCTAssertTrue(WarrenForumActivityClient.seen(fromEnvelope: #"{"ok":true}"#))
        for envelope in [nil, "", "not json", #"{"ok":false,"error":"error","reason":"http"}"#] {
            XCTAssertFalse(WarrenForumActivityClient.seen(fromEnvelope: envelope), envelope ?? "nil")
        }
    }
}
