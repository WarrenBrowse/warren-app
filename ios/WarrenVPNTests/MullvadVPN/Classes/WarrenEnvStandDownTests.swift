//
//  WarrenEnvStandDownTests.swift
//  WarrenVPNTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import Foundation
import XCTest

@testable import WarrenMockData
@testable import WarrenREST
@testable import WarrenRustRuntime
@testable import WarrenSettings
@testable import WarrenTypes
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

    /// The presence of the other install is a URL scheme any app on the device
    /// can register, so observing it may only ever OFFER the stand-down: the
    /// tunnel, the on-demand rule and "Force all apps" are untouched until the
    /// user says so.
    func testAnObservedInstallOnlyOffersTheStandDown() throws {
        let store = FakeEnvStandDownStore()
        let device = DeviceRecorder()
        device.installedSchemes = ["warren"]
        device.killSwitchIsOn = true
        let standDown = makeStandDown(store: store, device: device, higherPriority: try higherPriorityRows())

        standDown.refresh()

        XCTAssertEqual(device.steps, [])
        XCTAssertTrue(device.killSwitchIsOn)
        XCTAssertTrue(standDown.record.isOfferingStandDown)
        XCTAssertFalse(standDown.isStandingDown)
        XCTAssertEqual(store.warrenEnvStandDown.higherEnvironmentSeen, "prod")
    }

    /// The order is the safety: what is about to be given up is recorded
    /// first, the tunnel comes down while the kill switch is still armed, and
    /// only a tunnel that is actually down releases the block.
    func testTheAcceptedStandDownRecordsThenStopsThenDisarms() throws {
        let store = FakeEnvStandDownStore()
        let device = DeviceRecorder()
        device.installedSchemes = ["warren"]
        device.killSwitchIsOn = true
        let standDown = makeStandDown(store: store, device: device, higherPriority: try higherPriorityRows())

        standDown.refresh()
        standDown.confirmStandDown()

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

    /// Turning the offer down is the answer to a presence this build cannot
    /// authenticate: the record goes sticky and not one device call is made.
    func testTurningTheOfferDownTouchesNothing() throws {
        let store = FakeEnvStandDownStore()
        let device = DeviceRecorder()
        device.installedSchemes = ["warren"]
        device.killSwitchIsOn = true
        let standDown = makeStandDown(store: store, device: device, higherPriority: try higherPriorityRows())

        standDown.refresh()
        standDown.reEnable()

        XCTAssertEqual(device.steps, [])
        XCTAssertTrue(device.killSwitchIsOn)
        XCTAssertFalse(standDown.isStandingDown)
        XCTAssertFalse(standDown.record.isOfferingStandDown)
        XCTAssertTrue(store.warrenEnvStandDown.reEnabled)
    }

    /// A second environment taking over from the first keeps the two values
    /// the first stand-down recorded: the kill switch is already off, so
    /// reading it again would save the neutralised value as the one to put
    /// back and destroy the user's setting. The desktop daemon threads the
    /// held record through `stand_down_plan` for exactly this.
    func testASecondEnvironmentKeepsWhatTheFirstStandDownRecorded() throws {
        let store = FakeEnvStandDownStore()
        let device = DeviceRecorder()
        device.installedSchemes = ["warren-staging"]
        device.killSwitchIsOn = true
        let standDown = makeStandDown(store: store, device: device, higherPriority: try higherPriorityRows())

        standDown.refresh()
        standDown.confirmStandDown()
        device.finishStoppingTunnel()
        XCTAssertFalse(device.killSwitchIsOn)

        device.installedSchemes = ["warren", "warren-staging"]
        standDown.refresh()

        XCTAssertEqual(store.warrenEnvStandDown.higherEnvironmentSeen, "prod")
        XCTAssertTrue(store.warrenEnvStandDown.restoreIncludeAllNetworks)
        XCTAssertTrue(standDown.isStandingDown)

        standDown.reEnable()

        XCTAssertTrue(device.killSwitchIsOn)
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
        standDown.confirmStandDown()
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
        standDown.confirmStandDown()
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

    /// The higher install is gone: what this build gave up for it goes back
    /// first, and only then is the record dropped whole, so a later reinstall
    /// reads as the new transition it is. Dropping it without restoring would
    /// leave "Force all apps" off for good, with no banner left to say so and
    /// no record left to restore from.
    func testTheHigherEnvironmentGoingAwayPutsTheKillSwitchBack() throws {
        let store = FakeEnvStandDownStore()
        let device = DeviceRecorder()
        device.installedSchemes = ["warren"]
        device.killSwitchIsOn = true
        let standDown = makeStandDown(store: store, device: device, higherPriority: try higherPriorityRows())
        standDown.refresh()
        standDown.confirmStandDown()
        device.finishStoppingTunnel()
        XCTAssertFalse(device.killSwitchIsOn)

        device.installedSchemes = []
        standDown.refresh()

        XCTAssertTrue(device.killSwitchIsOn)
        XCTAssertNil(store.warrenEnvStandDown.higherEnvironmentSeen)
        XCTAssertFalse(standDown.isStandingDown)

        device.installedSchemes = ["warren"]
        standDown.refresh()

        XCTAssertTrue(standDown.record.isOfferingStandDown)
    }

    /// An offer the user never answered has changed nothing, so the install
    /// going away has nothing to put back.
    func testTheHigherEnvironmentGoingAwayAfterAnUnansweredOfferTouchesNothing() throws {
        let store = FakeEnvStandDownStore()
        let device = DeviceRecorder()
        device.installedSchemes = ["warren"]
        device.killSwitchIsOn = true
        let standDown = makeStandDown(store: store, device: device, higherPriority: try higherPriorityRows())
        standDown.refresh()

        device.installedSchemes = []
        standDown.refresh()

        XCTAssertEqual(device.steps, [])
        XCTAssertTrue(device.killSwitchIsOn)
        XCTAssertNil(store.warrenEnvStandDown.higherEnvironmentSeen)
    }

    /// A record written before the stand-down was put behind a confirmation
    /// carries no `standDownApplied`, and back then having seen an environment
    /// WAS having stood down for it. It has to keep reading that way, or the
    /// upgrade forgets that the kill switch is already off.
    func testARecordWrittenBeforeTheConfirmationStillReadsAsStoodDown() throws {
        let stored = """
            {"higherEnvironmentSeen":"prod","reEnabled":false,"restoreIncludeAllNetworks":true}
            """

        let record = try JSONDecoder().decode(
            WarrenEnvStandDownRecord.self,
            from: Data(stored.utf8)
        )

        XCTAssertTrue(record.isStandingDown)
        XCTAssertFalse(record.isOfferingStandDown)
        XCTAssertTrue(record.restoreIncludeAllNetworks)
    }

    /// The connect screen has to learn that the record moved, and it cannot
    /// learn it from a settings write: standing down and re-enabling both
    /// write the kill switch, and a write that changes nothing (the common
    /// case, "Force all apps" already off) never reaches the settings
    /// observer. Connect would then stay greyed out with no banner left.
    func testTheRecordMovingIsAnnouncedToTheConnectScreen() throws {
        let store = FakeEnvStandDownStore()
        let device = DeviceRecorder()
        device.installedSchemes = ["warren"]
        let standDown = makeStandDown(store: store, device: device, higherPriority: try higherPriorityRows())
        let announced = expectation(
            forNotification: .warrenEnvStandDownDidChange,
            object: nil,
            handler: nil
        )

        standDown.refresh()

        wait(for: [announced], timeout: 2)
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

    // MARK: the connect screen

    private func connectScreenModel() -> ConnectionViewViewModel {
        ConnectionViewViewModel(
            tunnelStatus: TunnelStatus(state: .disconnected),
            relayConstraints: RelayConstraints(),
            relayCache: MockRelayCache(),
            customListRepository: CustomListRepository()
        )
    }

    /// The tunnel refuses to arm while the record stands, so a pressable
    /// Connect could only ever answer with a refusal the user never sees: the
    /// stand-down banner outranks every other, and it is what carries the way
    /// back.
    func testTheConnectScreenTurnsItsButtonsOffWhileStandingDown() {
        let viewModel = connectScreenModel()

        viewModel.isStandingDownForHigherEnvironment = true

        XCTAssertTrue(viewModel.disableButtons)
    }

    func testTheConnectScreenComesBackOnceThisBuildIsReEnabled() {
        let viewModel = connectScreenModel()
        viewModel.isStandingDownForHigherEnvironment = true

        viewModel.isStandingDownForHigherEnvironment = false

        XCTAssertFalse(viewModel.disableButtons)
        guard case .connect = viewModel.actionButton else {
            return XCTFail("The action button is not Connect")
        }
    }
}
