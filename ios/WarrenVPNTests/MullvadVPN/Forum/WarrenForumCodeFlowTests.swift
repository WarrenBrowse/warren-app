//
//  WarrenForumCodeFlowTests.swift
//  WarrenVPNTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import WarrenRustRuntime
import XCTest

@testable import WarrenVPN

/// Where a typed forum code is routed, and what the journal says about it.
///
/// The three forum flows had no iOS test at all: the copy and the link parsing
/// were covered, the routing was not, while Android tested the same decisions
/// in `WarrenForumCodeUseCaseTest`. The routing is what decides whether a
/// support code opens the attach consent or the sign-in consent, and the
/// journal is what the staff reads afterwards to see which it was.
///
/// Both sub-flows are given a presenter returning nil, so `present` bails
/// before any UIKit work while `route` still writes its lines.
@MainActor
final class WarrenForumCodeFlowTests: XCTestCase {
    private var directory: URL!
    private var journal: WarrenForumEventsJournal!

    /// A 32-character session id, the only shape a code may have.
    private let sid = String(repeating: "a1b2c3d4", count: 4)

    // The async overrides, not `setUpWithError`: this class is `@MainActor`
    // and the throwing overrides are nonisolated, so they ran off the actor
    // and the properties were never set for the test body.
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

    private func flow(
        placement: WarrenForumCodePlacement?,
        probeDelay: TimeInterval = 0,
        probeTimeout: TimeInterval = 1
    ) -> WarrenForumCodeFlow {
        let login = WarrenForumLoginFlow()
        login.presenter = { nil }
        let attach = WarrenForumAttachFlow(journal: journal)
        attach.presenter = { nil }
        return WarrenForumCodeFlow(
            journal: journal,
            loginFlow: login,
            attachFlow: attach,
            probe: { _, _ -> WarrenForumCodePlacement in
                if probeDelay > 0 { Thread.sleep(forTimeInterval: probeDelay) }
                // A broker that does not answer in time is spelled as a probe
                // that sleeps past the timeout, since the seam cannot return
                // "no answer".
                return placement ?? .unknown
            },
            probeTimeout: probeTimeout
        )
    }

    /// The journal's own lines, decoded. It writes one JSON object per line,
    /// whose values are mostly strings but not all of them (`seq` is a number),
    /// so every value is rendered as a string for comparison.
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

    /// Submits a code that is expected to be accepted, and returns the journal
    /// once the routing behind it is done.
    @discardableResult
    private func route(
        code: String,
        placement: WarrenForumCodePlacement?,
        probeDelay: TimeInterval = 0,
        probeTimeout: TimeInterval = 1
    ) throws -> [[String: String]] {
        let flow = self.flow(
            placement: placement, probeDelay: probeDelay, probeTimeout: probeTimeout)
        let placed = expectation(description: "the screen was told it may leave")
        var onMainThread = false
        let accepted = flow.submit(code: code) {
            onMainThread = Thread.isMainThread
            placed.fulfill()
        }
        XCTAssertTrue(accepted, "\(code.debugDescription) was refused")
        wait(for: [placed], timeout: 5)
        XCTAssertTrue(onMainThread, "the screen was released off the main thread")
        return try entries()
    }

    func testACodeThatIsNotASessionIdIsRefusedWithoutProbing() throws {
        let flow = self.flow(placement: .login)
        for code in ["", "too-short", String(repeating: "z", count: 31)] {
            let accepted = flow.submit(code: code) {
                XCTFail("a refused code released the screen")
            }
            XCTAssertFalse(accepted, "\(code.debugDescription) was accepted")
        }
        XCTAssertTrue(try entries().isEmpty, "a refused code wrote to the journal")
    }

    func testASessionIdIsAcceptedAndRouted() throws {
        XCTAssertEqual(try route(code: sid, placement: .login).count, 1)
    }

    func testAnAttachPlacementRoutesToTheAttachConsent() throws {
        let entry = try XCTUnwrap(try route(code: sid, placement: .attach(topicId: 4242)).first)
        XCTAssertEqual(entry["event"], "link.received")
        XCTAssertEqual(entry["verdict"], "accepted")
        XCTAssertEqual(entry["source"], "typed-code")
        XCTAssertEqual(entry["kind"], "attach")
        XCTAssertEqual(entry["class"], "attach")
        XCTAssertEqual(entry["pre_topic"], "false")
    }

    /// A report still being composed has no topic number yet, and the journal
    /// has to say so or the staff cannot tell it from a lost topic id.
    func testAnAttachPlacementForAReportStillBeingComposedSaysSo() throws {
        let entry = try XCTUnwrap(try route(code: sid, placement: .attach(topicId: 0)).first)
        XCTAssertEqual(entry["pre_topic"], "true")
    }

    func testEveryOtherPlacementRoutesToTheLoginConsentUnderItsOwnClass() throws {
        let cases: [(WarrenForumCodePlacement, String)] = [
            (.login, "login"),
            (.attachWithoutTopic, "attach-no-topic"),
            (.gone, "gone"),
            (.unknown, "unknown"),
        ]
        for (placement, expected) in cases {
            let entry = try XCTUnwrap(try route(code: sid, placement: placement).last)
            XCTAssertEqual(entry["kind"], "login", "\(placement)")
            XCTAssertEqual(entry["class"], expected, "\(placement)")
        }
    }

    /// A broker that does not answer in time is not waited for: the login
    /// consent is raised under the class `timeout`, and the late answer is
    /// dropped rather than routed after the fact.
    func testAProbeThatOutlastsItsTimeoutIsAbandonedUnderTheTimeoutClass() throws {
        let entry = try XCTUnwrap(
            try route(
                code: sid,
                placement: .attach(topicId: 7),
                probeDelay: 0.5,
                probeTimeout: 0.05
            ).last)
        XCTAssertEqual(entry["kind"], "login")
        XCTAssertEqual(entry["class"], "timeout")

        // The late answer must not route a second time.
        Thread.sleep(forTimeInterval: 0.8)
        XCTAssertEqual(
            try entries().count, 1,
            "a probe answering after its timeout routed the code anyway")
    }
}
