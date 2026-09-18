//
//  TunnelMonitorTests.swift
//  PacketTunnelCoreTests
//
//  Created by pronebird on 15/08/2023.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import WarrenTypes
import Network
import XCTest

@testable import WarrenMockData
@testable import PacketTunnelCore

final class TunnelMonitorTests: XCTestCase {
    let networkCounters = NetworkCounters()

    func testShouldDetermineConnectionEstablished() async throws {
        let connectedExpectation = expectation(description: "Should report connected.")
        let connectionLostExpectation = expectation(description: "Should not report connection loss")
        connectionLostExpectation.isInverted = true

        let pinger = PingerMock(networkStatsReporting: networkCounters) { _, _ in
            return .sendReply()
        }

        let tunnelMonitor = createTunnelMonitor(pinger: pinger, timings: TunnelMonitorTimings())

        tunnelMonitor.onEvent = { event in
            switch event {
            case .connectionEstablished:
                connectedExpectation.fulfill()

            case .connectionLost:
                connectionLostExpectation.fulfill()
            }
        }

        tunnelMonitor.start(probeAddress: .loopback)

        await fulfillment(of: [connectedExpectation, connectionLostExpectation], timeout: .UnitTest.invertedTimeout)
    }

    func testInitialConnectionTimings() async throws {
        // Setup pinger so that it never receives any replies.
        let pinger = PingerMock(networkStatsReporting: networkCounters) { _, _ in .ignore }

        let timings = TunnelMonitorTimings(
            pingTimeout: .milliseconds(300),
            initialEstablishTimeout: .milliseconds(100),
            connectivityCheckInterval: .milliseconds(100)
        )

        let tunnelMonitor = createTunnelMonitor(pinger: pinger, timings: timings)

        var expectedTimings = [
            timings.initialEstablishTimeout.milliseconds,
            timings.initialEstablishTimeout.milliseconds * 2,
            timings.pingTimeout.milliseconds,
            timings.pingTimeout.milliseconds,
        ]

        // Calculate the amount of time necessary to perform the test. The
        // leeway is generous because the upper bound here is the machine's,
        // not the monitor's: a shared CI runner takes a multiple of a 100 ms
        // window just in scheduling jitter.
        var timeout = expectedTimings.reduce(0, +)
        timeout *= 8

        let expectation = expectation(description: "Should respect all timings.")
        expectation.expectedFulfillmentCount = expectedTimings.count

        // This date will be used to measure the amount of time elapsed between `.connectionLost` events.
        var startDate = Date()

        tunnelMonitor.onEvent = { [weak tunnelMonitor] event in
            guard case .connectionLost = event else { return }

            switch event {
            case .connectionLost:
                XCTAssertFalse(expectedTimings.isEmpty)

                let expectedDuration = expectedTimings.removeFirst()

                // Compute amount of time elapsed between `.connectionLost` events.
                let timeElapsed = Int(Date().timeIntervalSince(startDate) * 1000)

                // A LOWER bound only. Reporting the loss early is the defect
                // this test exists to catch: the monitor would be giving up
                // before the timeout it was configured with. Reporting it late
                // is the machine, and pinning an upper bound made this test
                // pass on a fast workstation and fail on a shared CI runner,
                // where scheduling jitter alone exceeded the window (200 ms
                // expected, 559 ms observed). The 10 ms of slack absorbs timer
                // coalescing, which can fire a hair before the deadline.
                XCTAssertGreaterThanOrEqual(
                    timeElapsed,
                    expectedDuration - 10,
                    "Reported connection loss after \(timeElapsed) ms, before the "
                        + "\(expectedDuration) ms it was configured to wait."
                )

                expectation.fulfill()

                if !expectedTimings.isEmpty {
                    startDate = Date()

                    // Continue monitoring by calling start() again.
                    tunnelMonitor?.start(probeAddress: .loopback)
                }

            case .connectionEstablished:
                XCTFail("Connection should fail.")
            }
        }

        // Start monitoring.
        tunnelMonitor.start(probeAddress: .loopback)

        await fulfillment(of: [expectation], timeout: TimeInterval(timeout) / 1000)
    }
}

extension TunnelMonitorTests {
    private func createTunnelMonitor(pinger: PingerProtocol, timings: TunnelMonitorTimings) -> TunnelMonitor {
        return TunnelMonitor(
            eventQueue: .main,
            pinger: pinger,
            tunnelDeviceInfo: TunnelDeviceInfoStub(networkStatsProviding: networkCounters),
            timings: timings
        )
    }
}
