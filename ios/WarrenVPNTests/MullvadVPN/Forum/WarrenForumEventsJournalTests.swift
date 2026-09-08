//
//  WarrenForumEventsJournalTests.swift
//  WarrenVPNTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import XCTest

@testable import WarrenVPN

/// The forum flows' event journal: the line shape the staff read off a
/// problem report, and the field grammar that leaves no room for a session
/// id, an address or a handle. The same grammar as Android's
/// `ForumEventsJournal`, so one reading applies to both reports.
final class WarrenForumEventsJournalTests: XCTestCase {
    private var directory: URL!

    override func setUpWithError() throws {
        try super.setUpWithError()
        directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("forum-journal-\(UUID().uuidString)", isDirectory: true)
    }

    override func tearDownWithError() throws {
        try? FileManager.default.removeItem(at: directory)
        try super.tearDownWithError()
    }

    func testALineCarriesTheSequenceTheTimeTheEventAndTheFieldsInOrder() throws {
        let at = try XCTUnwrap(ISO8601DateFormatter().date(from: "2026-09-08T19:33:53Z"))
        let line = WarrenForumEventsJournal.format(
            at: at, sequence: 7, event: .attachResult,
            fields: [.class("attached"), .elapsedMs(1234), .gzBytes(56_789), .preTopic(true)])
        XCTAssertEqual(
            line,
            #"{"seq":7,"at":"2026-09-08T19:33:53Z","event":"attach.result","class":"attached","elapsed_ms":"1234","gz_bytes":"56789","pre_topic":"true"}"#
        )
    }

    func testAClassTokenIsShorterThanASessionIdAndAnythingElseIsMalformed() {
        XCTAssertEqual(JournalField.classToken("deferred-connecting"), "deferred-connecting")
        XCTAssertEqual(JournalField.classToken("wrong-scheme:warren-beta"), "wrong-scheme:warren-beta")
        XCTAssertEqual(JournalField.classToken("http-502"), "http-502")
        // A session id (32 hex) and an SS58 address (49 chars) cannot pass,
        // even through a wrong call site.
        XCTAssertEqual(JournalField.classToken("0123456789abcdef0123456789abcdef"), JournalField.malformed)
        XCTAssertEqual(JournalField.classToken("wb7kgy8FF4rxhP9DnBmX3x6pWq2vN8kLmZ4tRj7yUcE5sGa2Q"), JournalField.malformed)
        XCTAssertEqual(JournalField.classToken("Has Spaces"), JournalField.malformed)
        XCTAssertEqual(JournalField.classToken(""), JournalField.malformed)
        XCTAssertEqual(JournalField.class("0123456789abcdef0123456789abcdef").value, JournalField.malformed)
    }

    func testRecordAppendsOneLinePerEventToTheJournalFile() throws {
        let journal = WarrenForumEventsJournal(directory: directory)
        journal.record(.linkReceived, .verdict("accepted"), .source(.deepLink), .kind(.attach), .coldStart(false))
        journal.record(.attachDeclined)
        let lines = try journal.drain()
        XCTAssertEqual(lines.count, 2)
        XCTAssertTrue(lines[0].contains(#""seq":0"#), lines[0])
        XCTAssertTrue(lines[0].contains(#""event":"link.received""#), lines[0])
        XCTAssertTrue(lines[0].contains(#""source":"deep-link""#), lines[0])
        XCTAssertTrue(lines[0].contains(#""kind":"attach""#), lines[0])
        XCTAssertTrue(lines[1].contains(#""seq":1"#), lines[1])
        XCTAssertTrue(lines[1].hasSuffix(#""event":"attach.declined"}"#), lines[1])
        XCTAssertEqual(journal.fileURL.lastPathComponent, WarrenForumEventsJournal.fileName)
    }

    func testTheJournalKeepsItsNewestHalfPastTheCap() throws {
        let journal = WarrenForumEventsJournal(directory: directory)
        // Well past the cap: every line is about 90 bytes.
        let count = Int(WarrenForumEventsJournal.maxBytes) / 60
        for _ in 0..<count {
            journal.record(.loginResult, .class("expired"))
        }
        journal.record(.loginResult, .class("approved"))
        let lines = try journal.drain()
        XCTAssertLessThan(lines.count, count, "the head must have been dropped")
        XCTAssertGreaterThan(lines.count, count / 4, "and only the head")
        XCTAssertTrue(try XCTUnwrap(lines.last).contains(#""class":"approved""#), "the newest line survives")
    }
}
