//
//  WarrenForumFlowJournalTests.swift
//  WarrenVPNTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import WarrenRustRuntime
import XCTest

@testable import WarrenVPN

/// What the attach and login flows record when a link reaches them.
///
/// The journal is what support staff reads to see why an attach or a sign-in
/// did not happen, and neither flow's recording had an iOS test: Android covers
/// the same decisions in `WarrenForumAttachUseCaseTest` and
/// `ForumAttachControllerTest`. A link whose verdict is written wrong, or not
/// written at all, leaves a report nobody can trace.
///
/// The presenter returns nil throughout, so `present` bails before any UIKit
/// work while the recording still runs.
@MainActor
final class WarrenForumFlowJournalTests: XCTestCase {
    private var directory: URL!
    private var journal: WarrenForumEventsJournal!

    private var host: String { WarrenProductAnchors.current.connectHost }
    private var scheme: String { WarrenProductAnchors.current.deepLinkScheme }
    private let sid = "0123456789abcdef0123456789abcdef"

    override func setUp() async throws {
        try await super.setUp()
        directory = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        journal = WarrenForumEventsJournal(directory: directory)
    }

    override func tearDown() async throws {
        try? FileManager.default.removeItem(at: directory)
        directory = nil
        journal = nil
        try await super.tearDown()
    }

    private func entries() throws -> [[String: String]] {
        journal.flush()
        return try journal.drain().compactMap { line in
            guard
                let data = line.data(using: .utf8),
                let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
            else { return nil }
            return object.mapValues { "\($0)" }
        }
    }

    private func attachFlow() -> WarrenForumAttachFlow {
        let flow = WarrenForumAttachFlow(journal: journal)
        flow.presenter = { nil }
        return flow
    }

    // MARK: - Attach

    func testAWellFormedAttachLinkIsRecordedAsAccepted() throws {
        let url = URL(string: "\(scheme)://attach-logs?sid=\(sid)&host=\(host)&topic=4242")!
        attachFlow().handle(url: url, coldStart: false)

        let entry = try XCTUnwrap(try entries().first)
        XCTAssertEqual(entry["event"], "link.received")
        XCTAssertEqual(entry["verdict"], "accepted")
        XCTAssertEqual(entry["source"], "deep-link")
        XCTAssertEqual(entry["kind"], "attach")
        XCTAssertEqual(entry["pre_topic"], "false")
        XCTAssertEqual(entry["cold_start"], "false")
    }

    /// A link that arrives with the app, rather than while it is running, is
    /// the case staff most often has to reason about, so it is recorded apart.
    func testALinkThatArrivedWithTheAppSaysSo() throws {
        let url = URL(string: "\(scheme)://attach-logs?sid=\(sid)&host=\(host)&topic=1")!
        attachFlow().handle(url: url, coldStart: true)
        XCTAssertEqual(try XCTUnwrap(try entries().first)["cold_start"], "true")
    }

    func testAReportStillBeingComposedIsRecordedAsPreTopic() throws {
        let url = URL(string: "\(scheme)://attach-logs?sid=\(sid)&host=\(host)&topic=0")!
        attachFlow().handle(url: url, coldStart: false)
        XCTAssertEqual(try XCTUnwrap(try entries().first)["pre_topic"], "true")
    }

    /// A refused link is recorded under the reason it was refused, not as a
    /// silence: the whole point of the journal is that a link that went nowhere
    /// is still accounted for.
    func testARefusedAttachLinkIsRecordedUnderItsReason() throws {
        let flow = attachFlow()
        // A host the build may not talk to, which is the refusal that matters.
        flow.handle(
            url: URL(string: "\(scheme)://attach-logs?sid=\(sid)&host=evil.example&topic=1")!,
            coldStart: false)

        let entry = try XCTUnwrap(try entries().first)
        XCTAssertEqual(entry["kind"], "attach")
        XCTAssertNotEqual(entry["verdict"], "accepted")
        XCTAssertFalse(try XCTUnwrap(entry["verdict"]).isEmpty, "a refusal with no reason")
    }

    func testAnUnparseableAttachLinkIsStillAccountedFor() throws {
        attachFlow().handle(url: URL(string: "\(scheme)://attach-logs")!, coldStart: false)
        let entry = try XCTUnwrap(try entries().first)
        XCTAssertEqual(entry["kind"], "attach")
        XCTAssertNotEqual(entry["verdict"], "accepted")
    }

    // MARK: - Login

    /// The login flow answers whether a typed code is even a session id before
    /// anything else happens, and that answer is what the screen shows.
    func testALoginCodeIsAcceptedOnlyWhenItIsASessionId() {
        let flow = WarrenForumLoginFlow()
        flow.presenter = { nil }
        XCTAssertTrue(flow.handle(code: sid))
        XCTAssertTrue(
            flow.handle(code: "  \(sid.uppercased())  "),
            "a code pasted with spacing or in capitals is the same code")
        for code in ["", "short", String(repeating: "z", count: 31), "\(sid)0"] {
            XCTAssertFalse(flow.handle(code: code), "\(code.debugDescription) was accepted")
        }
    }
}
