//
//  LeakTests.swift
//  WarrenVPNUITests
//
//  Created by Niklas Berglund on 2024-05-31.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import XCTest

class LeakTests: LoggedInWithTimeUITestCase {
    static let capturedStreamStartTimestamp: Double = 8
    static let capturedStreamEndTimestamp: Double = 3

    override func tearDown() {
        FirewallClient().removeRules()
        super.tearDown()
    }

    /// Send UDP traffic to a host, connect to relay and make sure - while connected to relay -
    /// that no leaked traffic went directly to the host
    ///
    /// DECISION (2026-06-14): kept skipped. The assertion itself is sound for the
    /// Warren tunnel (no direct traffic to the target while connected), but it is
    /// expected to FAIL by design unless the tunnel is started with
    /// includeAllNetworks = true. On iOS, sockets opened BEFORE the packet tunnel
    /// comes up keep their original route and bypass the tunnel until
    /// includeAllNetworks forces all flows through it. This is a platform routing
    /// property, not a Warren-vs-Mullvad difference, so removing the skip would turn
    /// a real iOS limitation into a red test. To re-enable, start the tunnel with
    /// includeAllNetworks enabled (Settings -> Include all networks) before the
    /// connect step, then assert no leak. That requires a live sim + backend, so it
    /// stays skipped here.
    func testConnectionStartedBeforeTunnelShouldNotLeakOutside() throws {
        let skipReason = """
            Connections started before the packet tunnel will leak as long as
            includeAllNetworks is not set to true when starting the tunnel.
            Re-enable only after starting the Warren tunnel with includeAllNetworks
            enabled (see the DECISION note above this test).
            """
        try XCTSkipIf(true, skipReason)
        let targetIPAddress = Networking.getAlwaysReachableIPAddress()
        startPacketCapture()
        let trafficGenerator = TrafficGenerator(destinationHost: targetIPAddress, port: 80)
        trafficGenerator.startGeneratingUDPTraffic(interval: 1.0)

        TunnelControlPage(app)
            .tapConnectButton()

        allowAddVPNConfigurationsIfAsked()

        TunnelControlPage(app)
            .waitForConnectedLabel()

        // Keep the tunnel connection for a while
        RunLoop.current.run(until: .now + 30)

        TunnelControlPage(app)
            .tapDisconnectButton()

        trafficGenerator.stopGeneratingUDPTraffic()

        var capturedStreams = stopPacketCapture()
        // For now cut the beginning and and end of the stream to trim out the part where the tunnel connection was not up
        capturedStreams = PacketCaptureClient.trimPackets(
            streams: capturedStreams,
            secondsStart: Self.capturedStreamStartTimestamp,
            secondsEnd: Self.capturedStreamEndTimestamp
        )
        LeakCheck.assertNoLeaks(streams: capturedStreams, rules: [NoTrafficToHostLeakRule(host: targetIPAddress)])
    }

    /// Send UDP traffic to a host, connect to relay and then disconnect to intentionally leak traffic and make sure that the test catches the leak
    func testTrafficCapturedOutsideOfTunnelShouldLeak() throws {
        let targetIPAddress = Networking.getAlwaysReachableIPAddress()
        startPacketCapture()
        let trafficGenerator = TrafficGenerator(destinationHost: targetIPAddress, port: 80)
        trafficGenerator.startGeneratingUDPTraffic(interval: 1.0)

        TunnelControlPage(app)
            .tapConnectButton()

        allowAddVPNConfigurationsIfAsked()

        TunnelControlPage(app)
            .waitForConnectedLabel()

        RunLoop.current.run(until: .now + 2)

        TunnelControlPage(app)
            .tapDisconnectButton()

        // Give it some time to generate traffic outside of tunnel
        RunLoop.current.run(until: .now + 5)

        TunnelControlPage(app)
            .tapConnectButton()

        // Keep the tunnel connection for a while
        RunLoop.current.run(until: .now + 5)

        TunnelControlPage(app)
            .tapDisconnectButton()

        // Keep the capture open for a while
        RunLoop.current.run(until: .now + 15)
        trafficGenerator.stopGeneratingUDPTraffic()

        var capturedStreams = stopPacketCapture()
        // For now cut the beginning and and end of the stream to trim out the part where the tunnel connection was not up
        capturedStreams = PacketCaptureClient.trimPackets(
            streams: capturedStreams,
            secondsStart: Self.capturedStreamStartTimestamp,
            secondsEnd: Self.capturedStreamEndTimestamp
        )
        LeakCheck.assertLeaks(streams: capturedStreams, rules: [NoTrafficToHostLeakRule(host: targetIPAddress)])
    }
}
