//
//  StartTunnelOperationTests.swift
//  MullvadVPNTests
//
//  Created by Andrew Bulhak on 2024-02-02.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import WarrenSettings
import WarrenTypes
import Network
import Operations
import XCTest
@testable import WarrenVPN

@testable import WarrenMockData

class StartTunnelOperationTests: XCTestCase {
    // MARK: utility code for setting up tests

    let testQueue = DispatchQueue(label: "StartTunnelOperationTests.testQueue")
    let operationQueue = AsyncOperationQueue()

    let loggedInDeviceState = DeviceState.loggedIn(
        StoredAccountData(
            identifier: "",
            number: "",
            expiry: .distantFuture
        ),
        StoredDeviceData(
            creationDate: Date(),
            identifier: "",
            name: "",
            hijackDNS: false,
            ipv4Address: IPAddressRange(from: "127.0.0.1/32")!,
            ipv6Address: IPAddressRange(from: "::ff/64")!,
            wgKeyData: StoredWgKeyData(creationDate: Date(), privateKey: WireGuard.PrivateKey())
        )
    )

    func makeInteractor(deviceState: DeviceState, tunnelState: TunnelState? = nil) -> MockTunnelInteractor {
        let interactor = MockTunnelInteractor(
            isConfigurationLoaded: true,
            settings: LatestTunnelSettings(),
            deviceState: deviceState
        )
        if let tunnelState {
            interactor.tunnelStatus = TunnelStatus(state: tunnelState)
        }
        return interactor
    }

    // MARK: the tests

    func testFailsIfNotLoggedIn() throws {
        let expectation = expectation(description: "Start tunnel operation failed")
        let operation = StartTunnelOperation(
            dispatchQueue: testQueue,
            interactor: makeInteractor(deviceState: .loggedOut)
        ) { result in
            guard case .failure = result else {
                XCTFail("Operation returned \(result), not failure")
                return
            }
            expectation.fulfill()
        }

        operationQueue.addOperation(operation)
        wait(for: [expectation], timeout: .UnitTest.timeout)
    }

    func testSetsReconnectIfDisconnecting() {
        let interactor = makeInteractor(deviceState: loggedInDeviceState, tunnelState: .disconnecting(.nothing))
        nonisolated(unsafe) var tunnelStatus = TunnelStatus()
        interactor.onUpdateTunnelStatus = { status in tunnelStatus = status }
        let expectation = expectation(description: "Tunnel status set to reconnect")

        let operation = StartTunnelOperation(
            dispatchQueue: testQueue,
            interactor: interactor
        ) { _ in
            XCTAssertEqual(tunnelStatus.state, .disconnecting(.reconnect))
            expectation.fulfill()
        }
        operationQueue.addOperation(operation)
        wait(for: [expectation], timeout: .UnitTest.timeout)
    }

    func testStartsTunnelIfDisconnected() {
        let interactor = makeInteractor(deviceState: loggedInDeviceState, tunnelState: .disconnected)
        let expectation = expectation(description: "Make tunnel provider and start tunnel")
        let operation = StartTunnelOperation(
            dispatchQueue: testQueue,
            interactor: interactor
        ) { _ in
            XCTAssertNotNil(interactor.tunnel)
            XCTAssertNotNil(interactor.tunnel?.startDate)
            expectation.fulfill()
        }
        operationQueue.addOperation(operation)
        wait(for: [expectation], timeout: .UnitTest.timeout)
    }

    // MARK: coexistence with a higher-priority product environment

    private var standingDown: WarrenEnvStandDownRecord {
        WarrenEnvStandDownRecord(
            higherEnvironmentSeen: "prod",
            reEnabled: false,
            restoreIncludeAllNetworks: true,
            standDownApplied: true
        )
    }

    /// The incoherent state this refusal exists to forbid: a tunnel held
    /// under a banner that says this build stood down. The banner is not
    /// dismissed by connecting, so the connect has to be the one to give way.
    func testRefusesToConnectWhileThisBuildHasStoodDown() {
        let interactor = makeInteractor(deviceState: loggedInDeviceState, tunnelState: .disconnected)
        let record = standingDown
        let expectation = expectation(description: "Connect refused")

        let operation = StartTunnelOperation(
            dispatchQueue: testQueue,
            interactor: interactor,
            standDownRecord: { record }
        ) { result in
            guard case let .failure(error) = result else {
                XCTFail("Operation returned \(result), not a refusal")
                return
            }
            XCTAssertTrue(error is WarrenEnvStandDownRefusal)
            // No tunnel was made, so nothing wrote a configuration carrying an
            // armed on-demand rule either: the system cannot bring this build
            // back up behind the user's back.
            XCTAssertNil(interactor.tunnel)
            // The state and the banner agree: no tunnel, and the banner still up.
            XCTAssertTrue(WarrenEnvStandDownNotificationProvider.shouldDisplay(record: record))
            expectation.fulfill()
        }

        operationQueue.addOperation(operation)
        wait(for: [expectation], timeout: .UnitTest.timeout)
    }

    /// A logged-out build that has stood down is refused for the stand-down,
    /// not for the device state: the coexistence rule holds whatever else is
    /// wrong, and it must not be reachable around a login.
    func testTheRefusalOutranksTheDeviceStateCheck() {
        let interactor = makeInteractor(deviceState: .loggedOut, tunnelState: .disconnected)
        let expectation = expectation(description: "Connect refused")

        let operation = StartTunnelOperation(
            dispatchQueue: testQueue,
            interactor: interactor,
            standDownRecord: { self.standingDown }
        ) { result in
            guard case let .failure(error) = result else {
                XCTFail("Operation returned \(result), not a refusal")
                return
            }
            XCTAssertTrue(error is WarrenEnvStandDownRefusal)
            expectation.fulfill()
        }

        operationQueue.addOperation(operation)
        wait(for: [expectation], timeout: .UnitTest.timeout)
    }

    /// The user tapped the banner and asked for this build back. The record
    /// keeps the other install marked as seen, so the only thing that changed
    /// is the consent, and connecting has to work exactly as it always did.
    func testConnectsNormallyOnceTheUserHasReEnabledThisBuild() {
        let interactor = makeInteractor(deviceState: loggedInDeviceState, tunnelState: .disconnected)
        var record = standingDown
        record.reEnabled = true
        let expectation = expectation(description: "Make tunnel provider and start tunnel")

        let operation = StartTunnelOperation(
            dispatchQueue: testQueue,
            interactor: interactor,
            standDownRecord: { record }
        ) { result in
            guard case .success = result else {
                XCTFail("Operation returned \(result), not success")
                return
            }
            XCTAssertNotNil(interactor.tunnel)
            XCTAssertNotNil(interactor.tunnel?.startDate)
            XCTAssertFalse(WarrenEnvStandDownNotificationProvider.shouldDisplay(record: record))
            expectation.fulfill()
        }

        operationQueue.addOperation(operation)
        wait(for: [expectation], timeout: .UnitTest.timeout)
    }
}
