//
//  WarrenAppGroupEventsTests.swift
//  WarrenVPNTests
//
//  Created by Warren on 2026-05-21.
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  Unit tests for the `WarrenAppGroupEvents` observer that bridges
//  `UserDefaults.didChangeNotification` to typed `WarrenFailoverEvent`
//  values consumed by `TunnelViewController` (cf.
//  `.planning/session-c5-c6-integration-brief.md` §7).
//
//  Tests use an in-process `UserDefaults(suiteName:)` (not the App
//  Group) so they don't require the Packet Tunnel entitlement.
//

import Combine
import Foundation
import XCTest


@MainActor
final class WarrenAppGroupEventsTests: XCTestCase {
    private var suiteName: String!
    private var defaults: UserDefaults!
    private var cancellables: Set<AnyCancellable>!

    override func setUp() async throws {
        try await super.setUp()
        // Per-test unique suite name to keep test runs isolated even
        // when run concurrently.
        suiteName = "WarrenAppGroupEventsTests.\(UUID().uuidString)"
        defaults = UserDefaults(suiteName: suiteName)!
        defaults.removePersistentDomain(forName: suiteName)
        cancellables = Set<AnyCancellable>()
    }

    override func tearDown() async throws {
        defaults.removePersistentDomain(forName: suiteName)
        defaults = nil
        suiteName = nil
        cancellables = nil
        try await super.tearDown()
    }

    func testInitialStateIsEmpty() {
        let events = WarrenAppGroupEvents(suiteName: suiteName)
        XCTAssertNil(events.lastFailover, "Expected no failover on fresh defaults")
        XCTAssertFalse(events.obfuscationActive, "Expected obfuscation false on fresh defaults")
    }

    func testFailoverIsSurfacedAfterDefaultsWrite() async {
        let events = WarrenAppGroupEvents(suiteName: suiteName)
        let expectation = expectation(description: "lastFailover published")
        var observed: WarrenFailoverEvent?

        events.$lastFailover
            .dropFirst()  // drop initial nil
            .sink { event in
                guard let event else { return }
                observed = event
                expectation.fulfill()
            }
            .store(in: &cancellables)

        let occurredAt = Date()
        defaults.set("Switzerland", forKey: WarrenAppGroupKey.lastFailoverExit.rawValue)
        defaults.set(occurredAt, forKey: WarrenAppGroupKey.lastFailoverAt.rawValue)

        await fulfillment(of: [expectation], timeout: 2.0)
        XCTAssertEqual(observed?.country, "Switzerland")
        XCTAssertEqual(
            observed?.occurredAt.timeIntervalSinceReferenceDate ?? 0,
            occurredAt.timeIntervalSinceReferenceDate,
            accuracy: 0.001
        )
    }

    func testObfuscationActiveToggleIsObserved() async {
        let events = WarrenAppGroupEvents(suiteName: suiteName)
        let expectation = expectation(description: "obfuscation flipped to true")
        events.$obfuscationActive
            .dropFirst()
            .sink { active in
                if active {
                    expectation.fulfill()
                }
            }
            .store(in: &cancellables)

        defaults.set(true, forKey: WarrenAppGroupKey.obfuscationActive.rawValue)

        await fulfillment(of: [expectation], timeout: 2.0)
        XCTAssertTrue(events.obfuscationActive)
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

    func testEventResetsToNilWhenKeysAreCleared() async {
        let events = WarrenAppGroupEvents(suiteName: suiteName)

        // First write a failover.
        let firstAppeared = expectation(description: "first failover surfaced")
        var clearedExpectation: XCTestExpectation?
        events.$lastFailover
            .dropFirst()
            .sink { event in
                if event != nil {
                    firstAppeared.fulfill()
                } else {
                    clearedExpectation?.fulfill()
                }
            }
            .store(in: &cancellables)

        defaults.set("Norway", forKey: WarrenAppGroupKey.lastFailoverExit.rawValue)
        defaults.set(Date(), forKey: WarrenAppGroupKey.lastFailoverAt.rawValue)
        await fulfillment(of: [firstAppeared], timeout: 2.0)

        // Now clear and expect a nil republish.
        clearedExpectation = expectation(description: "failover cleared")
        defaults.removeObject(forKey: WarrenAppGroupKey.lastFailoverExit.rawValue)
        defaults.removeObject(forKey: WarrenAppGroupKey.lastFailoverAt.rawValue)
        await fulfillment(of: [clearedExpectation!], timeout: 2.0)
        XCTAssertNil(events.lastFailover)
    }
}
