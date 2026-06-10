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
@testable import WarrenMockData
@testable import WarrenREST
@testable import WarrenRustRuntime
@testable import WarrenSettings

/// No-op stand-in for the FFI-backed `WarrenQuinnAdapter` so the actor's
/// lifecycle can be unit-tested without standing up the Rust tunnel.
private final class MockWarrenQuinnAdapter: WarrenQuinnAdapting {
    private(set) var startCount = 0
    private(set) var stopCount = 0
    private(set) var reconnectCount = 0
    private(set) var lastStartConfig: WarrenTunnelConfig?
    func start(config: WarrenTunnelConfig) throws {
        startCount += 1
        lastStartConfig = config
    }
    func stop() { stopCount += 1 }
    func reconnect() { reconnectCount += 1 }
    func pause() {}
    func resume() {}
}

/// Empty in-memory settings store so `SettingsManager.readSettings()` (called
/// inside `WarrenQuinnActor.start()`) returns a not-found error the actor
/// swallows into defaults, instead of hitting the real App Group store whose
/// container URL is nil under XCTest (a force-unwrap crash).
private final class InMemoryTestSettingsStore: SettingsStore, @unchecked Sendable {
    private var storage: [SettingsKey: Data] = [:]
    enum StoreError: Error { case notFound }
    func read(key: SettingsKey) throws -> Data {
        guard let data = storage[key] else { throw StoreError.notFound }
        return data
    }
    func write(_ data: Data, for key: SettingsKey) throws { storage[key] = data }
    func delete(key: SettingsKey) throws { storage[key] = nil }
}

final class WarrenQuinnActorTests: XCTestCase {
    override func setUp() {
        super.setUp()
        // Route SettingsManager at the in-memory store so start()'s settings
        // reads don't touch the App Group container (nil under XCTest).
        SettingsManager.unitTestStore = InMemoryTestSettingsStore()
    }

    override func tearDown() {
        SettingsManager.unitTestStore = nil
        super.tearDown()
    }

    // MARK: - applyEvent updates currentState

    func test_applyEvent_connected_surfacesConnectedWithSelectedRelay() async {
        let actor = makeStartedActor()
        actor.applyEvent(.connected)
        let state = await actor.observedState
        guard case let .connected(conn) = state else {
            return XCTFail("Expected .connected, got \(state)")
        }
        // The connection state carries the relay we started with, so the UI
        // can render the real exit (not a blank "Connected").
        XCTAssertEqual(
            conn.selectedRelays.exit.hostname,
            RelaySelectorStub.selectedRelays.exit.hostname
        )
    }

    func test_applyEvent_failover_surfacesReconnecting() async {
        let actor = makeStartedActor()
        actor.applyEvent(.connected)
        actor.applyEvent(.failover(toExit: "Sweden"))
        let state = await actor.observedState
        guard case .reconnecting = state else {
            return XCTFail("Expected .reconnecting, got \(state)")
        }
    }

    func test_applyEvent_reconnecting_surfacesReconnecting() async {
        let actor = makeStartedActor()
        actor.applyEvent(.reconnecting)
        let state = await actor.observedState
        guard case .reconnecting = state else {
            return XCTFail("Expected .reconnecting, got \(state)")
        }
    }

    // MARK: - reconnect(to:) relay change

    func test_reconnect_toPreSelected_restartsTheAdapterForTheNewExit() async {
        let mock = MockWarrenQuinnAdapter()
        let actor = makeStartedActor(adapter: mock)
        XCTAssertEqual(mock.startCount, 1, "start() brings up the first session")

        actor.reconnect(to: .preSelected(RelaySelectorStub.selectedRelays), reconnectReason: .userInitiated)

        XCTAssertEqual(mock.stopCount, 1, "a relay change tears down the old session")
        XCTAssertEqual(mock.startCount, 2, "a relay change brings up a new session")
        XCTAssertEqual(mock.reconnectCount, 0, "a relay change is a restart, not a same-exit re-handshake")
    }

    func test_start_forcesSingleHop_evenWhenAMultihopEntryRelayIsSelected() async {
        // The Warren Quinn FFI does not consume a multi-hop entry relay yet.
        // The stub selection carries a non-nil entry, so this guards that the
        // actor still builds a single-hop FFI config: a user whose persisted
        // setting is multi-hop must never be silently single-hopped at the data
        // plane while believing otherwise. Remove this guard only when the FFI
        // and the relay descriptors actually establish a multi-hop circuit.
        XCTAssertNotNil(
            RelaySelectorStub.selectedRelays.entry,
            "the stub must select an entry relay or this guard is vacuous"
        )
        let mock = MockWarrenQuinnAdapter()
        _ = makeStartedActor(adapter: mock)
        XCTAssertEqual(mock.startCount, 1)
        XCTAssertNil(
            mock.lastStartConfig?.multiHopRelay,
            "the FFI config must force single-hop until multi-hop is wired"
        )
    }

    func test_reconnect_toCurrent_reHandshakesSameExitWithoutRestart() async {
        let mock = MockWarrenQuinnAdapter()
        let actor = makeStartedActor(adapter: mock)

        actor.reconnect(to: .current, reconnectReason: .userInitiated)

        XCTAssertEqual(mock.reconnectCount, 1, ".current re-handshakes via adapter.reconnect()")
        XCTAssertEqual(mock.stopCount, 0, ".current must not tear the session down")
        XCTAssertEqual(mock.startCount, 1, ".current must not start a second session")
    }

    func test_applyEvent_connected_withoutStart_staysInitial() async {
        // No start() means no captured connection context, so a stray
        // `.connected` event must not fabricate a relay.
        let actor = WarrenQuinnActor()
        actor.applyEvent(.connected)
        let state = await actor.observedState
        guard case .initial = state else {
            return XCTFail("Expected .initial without a start context, got \(state)")
        }
    }

    func test_applyEvent_disconnected_updatesObservedStateToDisconnected() async {
        let actor = WarrenQuinnActor()
        actor.applyEvent(.connected)
        actor.applyEvent(.disconnected)
        await assertObservedState(actor, equals: .disconnected)
    }

    // MARK: - setErrorState surfaces a blocked state

    func test_setErrorState_surfacesErrorObservedStateWithReason() async {
        let actor = WarrenQuinnActor()
        actor.setErrorState(reason: .deviceLoggedOut)
        let state = await actor.observedState
        guard case let .error(blocked) = state else {
            return XCTFail("Expected .error, got \(state)")
        }
        XCTAssertEqual(blocked.reason, .deviceLoggedOut)
    }

    func test_setErrorState_emitsErrorOnObservedStatesStream() async {
        let actor = WarrenQuinnActor()
        let stream = await actor.observedStates
        var iterator = stream.makeAsyncIterator()
        // Drain the initial snapshot (.disconnected) so the next value is
        // the transition we are asserting.
        _ = await iterator.next()
        actor.setErrorState(reason: .deviceRevoked)
        let next = await iterator.next()
        guard case let .error(blocked) = next else {
            return XCTFail("Expected .error emission, got \(String(describing: next))")
        }
        XCTAssertEqual(blocked.reason, .deviceRevoked)
    }

    // MARK: - NAT-PMP events are state-neutral

    func test_applyEvent_natPmpEvents_doNotChangeState() async {
        let actor = WarrenQuinnActor()
        // Use a real, currently-producible base state (.error) so the test
        // asserts NAT-PMP neutrality without depending on the pending
        // connected-state surfacing.
        actor.setErrorState(reason: .deviceRevoked)
        actor.applyEvent(.natPmpMapped(internalPort: 8080, externalPort: 51820, lifetime: 7200))
        actor.applyEvent(.natPmpRenewed(externalPort: 51820))
        actor.applyEvent(.natPmpFailed(reason: "test"))
        let state = await actor.observedState
        guard case let .error(blocked) = state, blocked.reason == .deviceRevoked else {
            return XCTFail("NAT-PMP events must be state-neutral, got \(state)")
        }
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
        // Drive a real, currently-producible state (.error) so the snapshot
        // assertion does not depend on the pending connected-state surfacing.
        actor.setErrorState(reason: .deviceRevoked)
        let stream = await actor.observedStates
        var iterator = stream.makeAsyncIterator()
        let first = await iterator.next()
        guard case .error = first else {
            return XCTFail("Expected first emission to be .error, got \(String(describing: first))")
        }
    }

    // MARK: - bindWalletSigningSeed + start flow

    /// An actor wired with a no-op adapter + seed and started against the
    /// stub relays, so it has captured a connection context and `.connected`
    /// events surface a real `ObservedConnectionState`.
    private func makeStartedActor(
        adapter: WarrenQuinnAdapting = MockWarrenQuinnAdapter()
    ) -> WarrenQuinnActor {
        let actor = WarrenQuinnActor()
        actor.bindAdapter(adapter)
        actor.bindWalletSigningSeed(Data(repeating: 0xAB, count: 32))
        actor.start(options: StartOptions(
            launchSource: .app,
            selectedRelays: RelaySelectorStub.selectedRelays
        ))
        return actor
    }

    func test_start_withoutSeedBound_doesNotCrash() async {
        let actor = WarrenQuinnActor()
        // No bindAdapter ; no bindWalletSigningSeed.
        // start(options:) should log + bail out cleanly.
        actor.start(options: StartOptions(launchSource: .app))
        await assertObservedState(actor, equals: .disconnected)
    }

    func test_bindWalletSigningSeed_storesAndStopClears() async {
        let actor = WarrenQuinnActor()
        let seed = Data(repeating: 0xAB, count: 32)
        actor.bindWalletSigningSeed(seed)

        // stop() should clear it ; we can't introspect directly but
        // we can verify the actor stays usable after stop.
        actor.stop()
        await assertObservedState(actor, equals: .disconnected)

        // Re-binding seed after stop should work (idempotent).
        actor.bindWalletSigningSeed(seed)
        await assertObservedState(actor, equals: .disconnected)
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
