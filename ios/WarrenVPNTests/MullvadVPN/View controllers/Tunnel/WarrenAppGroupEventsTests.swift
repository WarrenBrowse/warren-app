//
//  WarrenAppGroupEventsTests.swift
//  WarrenVPNTests
//
//  Created by Warren on 2026-05-21.
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  Unit tests for the `WarrenAppGroupEvents` bridge that turns the tunnel
//  extension's App Group writes into typed values for the main app.
//
//  These used to write through the same `UserDefaults` instance the bridge
//  observed and wait for `UserDefaults.didChangeNotification`. That notified
//  only because the write happened in the test's own process: the real
//  producer is the packet tunnel extension, and that notification is never
//  posted for a write from another process. So the suite was green while the
//  behaviour it described could not happen on a device.
//
//  Every test below therefore writes through a SECOND `UserDefaults` handle on
//  the same suite (the closest a unit test gets to a second process) and reads
//  the value back through the pull path production uses.
//

import Combine
import Foundation
import WarrenRustRuntime
import XCTest
@testable import WarrenVPN

@MainActor
final class WarrenAppGroupEventsTests: XCTestCase {
    private var suiteName: String!
    /// The producer's handle. Deliberately not the one the bridge resolves.
    private var producer: UserDefaults!

    override func setUp() async throws {
        try await super.setUp()
        // Per-test unique suite name to keep test runs isolated even
        // when run concurrently.
        suiteName = "WarrenAppGroupEventsTests.\(UUID().uuidString)"
        producer = UserDefaults(suiteName: suiteName)!
        producer.removePersistentDomain(forName: suiteName)
    }

    override func tearDown() async throws {
        producer.removePersistentDomain(forName: suiteName)
        producer = nil
        suiteName = nil
        try await super.tearDown()
    }

    func testInitialStateIsEmpty() {
        let events = WarrenAppGroupEvents(suiteName: suiteName)
        XCTAssertNil(events.lastFailover, "Expected no failover on fresh defaults")
        XCTAssertFalse(events.obfuscationActive, "Expected obfuscation false on fresh defaults")
        XCTAssertNil(events.lastPinMismatch)
    }

    func testAFailoverWrittenByAnotherHandleIsReadBack() {
        let events = WarrenAppGroupEvents(suiteName: suiteName)
        let occurredAt = Date()

        producer.set("Switzerland", forKey: WarrenAppGroupKey.lastFailoverExit.rawValue)
        producer.set(occurredAt, forKey: WarrenAppGroupKey.lastFailoverAt.rawValue)
        events.refresh()

        XCTAssertEqual(events.lastFailover?.country, "Switzerland")
        XCTAssertEqual(
            events.lastFailover?.occurredAt.timeIntervalSinceReferenceDate ?? 0,
            occurredAt.timeIntervalSinceReferenceDate,
            accuracy: 0.001
        )
        XCTAssertEqual(events.lastFailover?.isFresh, true)
    }

    func testTheObfuscationFlagIsReadBack() {
        let events = WarrenAppGroupEvents(suiteName: suiteName)

        producer.set(true, forKey: WarrenAppGroupKey.obfuscationActive.rawValue)
        events.refresh()

        XCTAssertTrue(events.obfuscationActive)
    }

    /// The pin mismatch is the one event with a security decision behind it, so
    /// it gets its own round trip through the JSON the extension writes.
    func testAPinMismatchWrittenByAnotherHandleIsReadBack() throws {
        let events = WarrenAppGroupEvents(suiteName: suiteName)
        let mismatch = WarrenPinMismatch(
            exitId: String(repeating: "1", count: 16),
            observed: String(repeating: "b", count: 64),
            pinned: String(repeating: "a", count: 64),
            country: "ch"
        )
        let json = try XCTUnwrap(String(data: try JSONEncoder().encode(mismatch), encoding: .utf8))

        producer.set(json, forKey: WarrenAppGroupKey.pinMismatch.rawValue)
        producer.set(Date(), forKey: WarrenAppGroupKey.pinMismatchAt.rawValue)
        events.refresh()

        XCTAssertEqual(events.lastPinMismatch?.mismatch, mismatch)
        XCTAssertEqual(events.lastPinMismatch?.isFresh, true)
    }

    /// The poll is the only trigger on a device, so it gets its own test rather
    /// than resting on `refresh()` being called by hand everywhere else.
    func testThePollPicksUpAWriteWithoutAnybodyAskingForIt() async throws {
        let events = WarrenAppGroupEvents(suiteName: suiteName)
        var seen: WarrenFailoverEvent?
        var cancellables = Set<AnyCancellable>()
        let published = expectation(description: "the poll surfaced the failover")
        events.$lastFailover
            .compactMap { $0 }
            .sink { event in
                seen = event
                published.fulfill()
            }
            .store(in: &cancellables)

        events.startPolling(every: 0.05)
        defer { events.stopPolling() }
        producer.set("Norway", forKey: WarrenAppGroupKey.lastFailoverExit.rawValue)
        producer.set(Date(), forKey: WarrenAppGroupKey.lastFailoverAt.rawValue)

        await fulfillment(of: [published], timeout: 2.0)
        XCTAssertEqual(seen?.country, "Norway")
    }

    func testClearingTheKeysClearsTheEvent() {
        let events = WarrenAppGroupEvents(suiteName: suiteName)
        producer.set("Norway", forKey: WarrenAppGroupKey.lastFailoverExit.rawValue)
        producer.set(Date(), forKey: WarrenAppGroupKey.lastFailoverAt.rawValue)
        events.refresh()
        XCTAssertNotNil(events.lastFailover)

        producer.removeObject(forKey: WarrenAppGroupKey.lastFailoverExit.rawValue)
        producer.removeObject(forKey: WarrenAppGroupKey.lastFailoverAt.rawValue)
        events.refresh()

        XCTAssertNil(events.lastFailover)
    }

    func testStaleFailoverIsNotFresh() {
        // A failover that happened > 30 seconds ago must report
        // `isFresh == false` so the banner suppression in
        // `TunnelViewController.subscribeToFailoverEvents` works.
        let staleEvent = WarrenFailoverEvent(
            country: "Sweden",
            occurredAt: Date().addingTimeInterval(-60)
        )
        XCTAssertFalse(staleEvent.isFresh, "Expected 60-s-old event to be stale")
    }

    func testRecentFailoverIsFresh() {
        let recent = WarrenFailoverEvent(country: "Germany", occurredAt: Date())
        XCTAssertTrue(recent.isFresh, "Expected just-now event to be fresh")
    }
}
