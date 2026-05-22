//
//  WarrenQuinnActorTests.swift
//  PacketTunnelCoreTests
//
//  Created by Warren on 2026-05-22.
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  Lifecycle tests for the C.4.3.X wire-up : verify `applyEvent`
//  drives the observedState snapshot + observedStates AsyncStream,
//  that `waitUntilDisconnected` blocks until a `.disconnected` event
//  fires, and that NAT-PMP events leave the tunnel state unchanged.
//

import XCTest

@testable import PacketTunnelCore
@testable import WarrenRustRuntime

final class WarrenQuinnActorTests: XCTestCase {
    // MARK: - applyEvent updates currentState

    func test_applyEvent_connected_updatesObservedStateToConnected() async {
        let actor = WarrenQuinnActor()
        await assertObservedState(actor, equals: .disconnected)
        actor.applyEvent(.connected)
        await assertObservedState(actor, equals: .connected)
    }

    func test_applyEvent_failover_updatesObservedStateToReconnecting() async {
        let actor = WarrenQuinnActor()
        actor.applyEvent(.connected)
        actor.applyEvent(.failover(toExit: "Sweden"))
        await assertObservedState(actor, equals: .reconnecting)
    }

    func test_applyEvent_reconnecting_updatesObservedStateToReconnecting() async {
        let actor = WarrenQuinnActor()
        actor.applyEvent(.reconnecting)
        await assertObservedState(actor, equals: .reconnecting)
    }

    func test_applyEvent_disconnected_updatesObservedStateToDisconnected() async {
        let actor = WarrenQuinnActor()
        actor.applyEvent(.connected)
        actor.applyEvent(.disconnected)
        await assertObservedState(actor, equals: .disconnected)
    }

    // MARK: - NAT-PMP events are state-neutral

    func test_applyEvent_natPmpEvents_doNotChangeState() async {
        let actor = WarrenQuinnActor()
        actor.applyEvent(.connected)
        actor.applyEvent(.natPmpMapped(internalPort: 8080, externalPort: 51820, lifetime: 7200))
        actor.applyEvent(.natPmpRenewed(externalPort: 51820))
        actor.applyEvent(.natPmpFailed(reason: "test"))
        await assertObservedState(actor, equals: .connected)
    }

    // MARK: - waitUntilDisconnected

    func test_waitUntilDisconnected_resolvesOnDisconnectedEvent() async {
        let actor = WarrenQuinnActor()
        actor.applyEvent(.connected)

        let waitExpectation = expectation(description: "waitUntilDisconnected resolves")
        Task {
            await actor.waitUntilDisconnected()
            waitExpectation.fulfill()
        }

        // Give the waiter a moment to install the continuation, then
        // fire the disconnected event.
        try? await Task.sleep(nanoseconds: 50_000_000)
        actor.applyEvent(.disconnected)

        await fulfillment(of: [waitExpectation], timeout: 2.0)
    }

    func test_waitUntilDisconnected_returnsImmediatelyIfAlreadyDisconnected() async {
        let actor = WarrenQuinnActor()
        // Default state is .disconnected ; should return immediately
        // without needing an event to fire.
        await actor.waitUntilDisconnected()
        // Reaching here means the function returned ; success.
        await assertObservedState(actor, equals: .disconnected)
    }

    // MARK: - observedStates stream

    func test_observedStates_emitsCurrentSnapshot_onFirstSubscription() async {
        let actor = WarrenQuinnActor()
        actor.applyEvent(.connected)
        let stream = await actor.observedStates
        var iterator = stream.makeAsyncIterator()
        let first = await iterator.next()
        switch first {
        case .connected:
            // expected
            break
        default:
            XCTFail("Expected first emission to be .connected, got \(String(describing: first))")
        }
    }

    // MARK: - Helpers

    private func assertObservedState(
        _ actor: WarrenQuinnActor,
        equals expected: ObservedState,
        file: StaticString = #filePath,
        line: UInt = #line
    ) async {
        let state = await actor.observedState
        switch (state, expected) {
        case (.disconnected, .disconnected),
            (.connecting, .connecting),
            (.connected, .connected),
            (.reconnecting, .reconnecting):
            return
        default:
            XCTFail(
                "Expected ObservedState \(expected), got \(state)",
                file: file,
                line: line
            )
        }
    }
}
