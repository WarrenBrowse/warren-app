//
//  WarrenEnvStandDownTests.swift
//  WarrenVPNTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import Foundation
import XCTest

@testable import WarrenRustRuntime
@testable import WarrenSettings
@testable import WarrenVPN

private final class FakeEnvStandDownStore: WarrenEnvStandDownStoring {
    var warrenEnvStandDown = WarrenEnvStandDownRecord()
}

/// Every device call the stand-down makes, recorded in the order it made them.
private enum StandDownStep: Equatable {
    case readKillSwitch
    case stopTunnel
    case killSwitch(Bool)
}

/// The device the stand-down acts on, replaced by a recorder so the ORDER of
/// the teardown can be asserted. The tunnel stop is deliberately not completed
/// until the test says so: the rule is that the kill switch stays armed until
/// the tunnel is actually down.
private final class DeviceRecorder {
    var installedSchemes: Set<String> = []
    var killSwitchIsOn = false
    private(set) var steps: [StandDownStep] = []
    private(set) var recordWhenTunnelStopped: WarrenEnvStandDownRecord?
    private var pendingCompletion: (() -> Void)?

    var store: FakeEnvStandDownStore?

    func device() -> WarrenEnvStandDown.Device {
        WarrenEnvStandDown.Device(
            isInstalled: { [unowned self] scheme in installedSchemes.contains(scheme) },
            stopTunnelAndClearOnDemand: { [unowned self] completion in
                steps.append(.stopTunnel)
                recordWhenTunnelStopped = store?.warrenEnvStandDown
                pendingCompletion = completion
            },
            readKillSwitch: { [unowned self] in
                steps.append(.readKillSwitch)
                return killSwitchIsOn
            },
            writeKillSwitch: { [unowned self] isOn in
                steps.append(.killSwitch(isOn))
                killSwitchIsOn = isOn
            }
        )
    }

    /// The tunnel finished coming down.
    func finishStoppingTunnel() {
        let completion = pendingCompletion
        pendingCompletion = nil
        completion?()
    }
}

final class WarrenEnvStandDownTests: XCTestCase {
    /// The real prod and staging rows, read from the shared client-rules
    /// fixture, so the scheme the stand-down looks for is the shipped one.
    private func higherPriorityRows() throws -> [WarrenProductAnchors] {
        let fixture = try ClientRulesFixtures.load("product_env.json")
        let environments = try ClientRulesFixtures.object(fixture, "environments")
        return try ["prod", "staging"].map { name in
            let row = try ClientRulesFixtures.object(environments, name)
            let data = try JSONSerialization.data(withJSONObject: row)
            return try XCTUnwrap(WarrenProductAnchors.decode(String(decoding: data, as: UTF8.self)))
        }
    }

    private func makeStandDown(
        store: FakeEnvStandDownStore,
        device: DeviceRecorder,
        higherPriority: [WarrenProductAnchors]
    ) -> WarrenEnvStandDown {
        device.store = store
        return WarrenEnvStandDown(
            store: store,
            higherPriority: higherPriority,
            device: device.device()
        )
    }

    func testNoHigherEnvironmentInstalledLeavesThisBuildAlone() throws {
        let store = FakeEnvStandDownStore()
        let device = DeviceRecorder()
        let standDown = makeStandDown(store: store, device: device, higherPriority: try higherPriorityRows())

        standDown.refresh()

        XCTAssertEqual(device.steps, [])
        XCTAssertFalse(standDown.isStandingDown)
        XCTAssertNil(store.warrenEnvStandDown.higherEnvironmentSeen)
    }

    /// Prod outranks everything, so its list is empty and it can never stand
    /// down, whatever else is on the device.
    func testAnEmptyHigherPriorityListNeverStandsDown() {
        let store = FakeEnvStandDownStore()
        let device = DeviceRecorder()
        device.installedSchemes = ["warren", "warren-beta", "warren-staging"]
        let standDown = makeStandDown(store: store, device: device, higherPriority: [])

        standDown.refresh()

        XCTAssertEqual(device.steps, [])
        XCTAssertFalse(standDown.isStandingDown)
    }

    /// The order is the safety: what is about to be given up is recorded
    /// first, the tunnel comes down while the kill switch is still armed, and
    /// only a tunnel that is actually down releases the block.
    func testTheFirstDetectionRecordsThenStopsThenDisarms() throws {
        let store = FakeEnvStandDownStore()
        let device = DeviceRecorder()
        device.installedSchemes = ["warren"]
        device.killSwitchIsOn = true
        let standDown = makeStandDown(store: store, device: device, higherPriority: try higherPriorityRows())

        standDown.refresh()

        // The tunnel has been asked to stop and the block still stands.
        XCTAssertEqual(device.steps, [.readKillSwitch, .stopTunnel])
        XCTAssertTrue(device.killSwitchIsOn)
        XCTAssertEqual(device.recordWhenTunnelStopped?.higherEnvironmentSeen, "prod")
        XCTAssertEqual(device.recordWhenTunnelStopped?.restoreIncludeAllNetworks, true)

        device.finishStoppingTunnel()

        XCTAssertEqual(device.steps, [.readKillSwitch, .stopTunnel, .killSwitch(false)])
        XCTAssertFalse(device.killSwitchIsOn)
        XCTAssertTrue(standDown.isStandingDown)
    }

    /// Presence, observed once. The other install is still there at every
    /// later launch, so a rule that re-evaluated itself would tear the build
    /// down again and undo the manual re-enable.
    func testASecondObservationOfTheSameInstallDoesNothing() throws {
        let store = FakeEnvStandDownStore()
        let device = DeviceRecorder()
        device.installedSchemes = ["warren"]
        let standDown = makeStandDown(store: store, device: device, higherPriority: try higherPriorityRows())

        standDown.refresh()
        device.finishStoppingTunnel()
        let afterFirst = device.steps

        standDown.refresh()

        XCTAssertEqual(device.steps, afterFirst)
    }

    func testReEnableRestoresTheRecordedKillSwitchAndClearsTheBanner() throws {
        let store = FakeEnvStandDownStore()
        let device = DeviceRecorder()
        device.installedSchemes = ["warren"]
        device.killSwitchIsOn = true
        let standDown = makeStandDown(store: store, device: device, higherPriority: try higherPriorityRows())

        standDown.refresh()
        device.finishStoppingTunnel()
        XCTAssertFalse(device.killSwitchIsOn)

        standDown.reEnable()

        XCTAssertTrue(device.killSwitchIsOn)
        XCTAssertFalse(standDown.isStandingDown)
        XCTAssertTrue(store.warrenEnvStandDown.reEnabled)
        // Sticky: the install stays marked as seen, so the next launch reads
        // no transition and leaves the user's choice standing.
        XCTAssertEqual(store.warrenEnvStandDown.higherEnvironmentSeen, "prod")

        standDown.refresh()

        XCTAssertFalse(standDown.isStandingDown)
        XCTAssertTrue(device.killSwitchIsOn)
    }

    /// The higher install is gone: the record is dropped whole, so a later
    /// reinstall reads as the new transition it is.
    func testTheHigherEnvironmentGoingAwayForgetsTheRecord() throws {
        let store = FakeEnvStandDownStore()
        let device = DeviceRecorder()
        device.installedSchemes = ["warren"]
        let standDown = makeStandDown(store: store, device: device, higherPriority: try higherPriorityRows())
        standDown.refresh()
        device.finishStoppingTunnel()

        device.installedSchemes = []
        standDown.refresh()

        XCTAssertNil(store.warrenEnvStandDown.higherEnvironmentSeen)
        XCTAssertFalse(standDown.isStandingDown)

        device.installedSchemes = ["warren"]
        standDown.refresh()

        XCTAssertTrue(standDown.isStandingDown)
    }

    /// Staging outranks beta too, and the strongest installed environment is
    /// the one the record names.
    func testTheStrongestInstalledEnvironmentIsTheOneRecorded() throws {
        let rows = try higherPriorityRows()
        let store = FakeEnvStandDownStore()
        let device = DeviceRecorder()
        device.installedSchemes = ["warren-staging"]
        let standDown = makeStandDown(store: store, device: device, higherPriority: rows)

        standDown.refresh()

        XCTAssertEqual(store.warrenEnvStandDown.higherEnvironmentSeen, "staging")

        device.installedSchemes = ["warren", "warren-staging"]
        let secondStore = FakeEnvStandDownStore()
        let secondDevice = DeviceRecorder()
        secondDevice.installedSchemes = ["warren", "warren-staging"]
        let second = makeStandDown(store: secondStore, device: secondDevice, higherPriority: rows)

        second.refresh()

        XCTAssertEqual(secondStore.warrenEnvStandDown.higherEnvironmentSeen, "prod")
    }
}
