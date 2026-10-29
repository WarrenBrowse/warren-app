//
//  DeviceManagementViewModelTests.swift
//  WarrenVPNTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import Foundation
import WarrenRustRuntime
import XCTest

@testable import WarrenVPN

/// The registry at its boundary: answers set per call, requests recorded.
private final class FakeRegistry: DeviceManaging, @unchecked Sendable {
    var list: Result<WarrenAccountDeviceList, WarrenDeviceRegistryError>
    var rename: Result<Bool, WarrenDeviceRegistryError> = .success(false)
    var revoke: Result<Bool, WarrenDeviceRegistryError> = .success(false)
    private(set) var requests: [String] = []

    init(list: Result<WarrenAccountDeviceList, WarrenDeviceRegistryError>) {
        self.list = list
    }

    func listDevices() async -> Result<WarrenAccountDeviceList, WarrenDeviceRegistryError> {
        requests.append("list")
        return list
    }

    func renameDevice(_ id: String, name: String) async -> Result<Bool, WarrenDeviceRegistryError> {
        requests.append("rename \(id) \(name)")
        return rename
    }

    func revokeDevice(_ id: String) async -> Result<Bool, WarrenDeviceRegistryError> {
        requests.append("revoke \(id)")
        return revoke
    }
}

@MainActor
final class DeviceManagementViewModelTests: XCTestCase {
    private let me = WarrenAccountDevice(
        id: "me", name: "brave otter", platform: "ios", appVersion: "1.1.40",
        lastSeenOn: WarrenRegistryDay("2026-10-01"), thisDevice: true
    )
    private let other = WarrenAccountDevice(
        id: "other", name: "quiet heron", platform: "windows", appVersion: "1.1.40",
        lastSeenOn: WarrenRegistryDay("2026-10-28"), thisDevice: false
    )

    private func model(_ registry: FakeRegistry) -> DeviceManagementViewModel {
        DeviceManagementViewModel(deviceManaging: registry, today: { WarrenRegistryDay("2026-10-28")! })
    }

    func testALoadedListShowsThisDeviceFirstAndTheOlderClientDay() async {
        let registry = FakeRegistry(
            list: .success(.init(devices: [other, me], olderClientSeenOn: WarrenRegistryDay("2026-10-25"))))
        let viewModel = model(registry)

        await viewModel.fetchDevices()

        XCTAssertEqual(viewModel.loadState, .loaded)
        XCTAssertEqual(viewModel.devices.map(\.id), ["me", "other"])
        XCTAssertEqual(viewModel.olderClientSeenOn, WarrenRegistryDay("2026-10-25"))
    }

    func testAListThatCouldNotBeLoadedOffersTheRetryWithoutEndingTheSession() async {
        let viewModel = model(FakeRegistry(list: .failure(.unavailable)))
        var revoked = false
        viewModel.onRevoked = { revoked = true }

        await viewModel.fetchDevices()

        XCTAssertEqual(viewModel.loadState, .failed)
        XCTAssertFalse(revoked)
    }

    /// The registry says another device revoked this one: the session ends
    /// now, not at the extension's next refresh.
    func testAListAnsweringRevokedTakesInTheRevocation() async {
        let viewModel = model(FakeRegistry(list: .failure(.revoked)))
        var revoked = false
        viewModel.onRevoked = { revoked = true }

        await viewModel.fetchDevices()

        XCTAssertTrue(revoked)
        XCTAssertEqual(viewModel.loadState, .failed)
    }

    func testAFailedDisconnectSaysSoAndReadsTheListAgain() async {
        let registry = FakeRegistry(list: .success(.init(devices: [me, other], olderClientSeenOn: nil)))
        registry.revoke = .failure(.unavailable)
        let viewModel = model(registry)

        await viewModel.disconnect(other)

        XCTAssertEqual(viewModel.failure, .disconnect)
        XCTAssertEqual(registry.requests, ["revoke other", "list"])
        XCTAssertEqual(viewModel.busy, [])
    }

    /// The device is already gone, which is what the user asked for.
    func testDisconnectingADeviceAlreadyGoneIsNoFailure() async {
        let registry = FakeRegistry(list: .success(.init(devices: [me], olderClientSeenOn: nil)))
        registry.revoke = .failure(.notFound)
        let viewModel = model(registry)

        await viewModel.disconnect(other)

        XCTAssertNil(viewModel.failure)
        XCTAssertEqual(viewModel.devices.map(\.id), ["me"])
    }

    func testARenameSendsTheTrimmedNameAndAFailedOneSaysSo() async {
        let registry = FakeRegistry(list: .success(.init(devices: [me, other], olderClientSeenOn: nil)))
        registry.rename = .failure(.unavailable)
        let viewModel = model(registry)

        await viewModel.rename(other, to: "  red cat ")

        XCTAssertEqual(registry.requests, ["rename other red cat", "list"])
        XCTAssertEqual(viewModel.failure, .rename)
    }

    func testANameTheRegistryWouldRefuseIsNeverSent() async {
        let registry = FakeRegistry(list: .success(.init(devices: [me, other], olderClientSeenOn: nil)))
        let viewModel = model(registry)

        await viewModel.rename(other, to: String(repeating: "x", count: 41))

        XCTAssertEqual(registry.requests, [])
    }

    /// "Log out" on this device revokes it in the registry, then the wallet
    /// leaves the device.
    func testLoggingOutThisDeviceRevokesItThenLogsOut() async {
        let registry = FakeRegistry(list: .success(.init(devices: [me, other], olderClientSeenOn: nil)))
        registry.revoke = .success(true)
        let viewModel = model(registry)
        var loggedOutAfter: [String] = []

        let loggedOut = await viewModel.logOutThisDevice(me) { loggedOutAfter = registry.requests }

        XCTAssertTrue(loggedOut)
        XCTAssertEqual(loggedOutAfter, ["revoke me"])
    }

    /// The registry kept the device, so the wallet stays and the user can try
    /// again, as on desktop and Android.
    func testAFailedRevocationOfThisDeviceKeepsTheWallet() async {
        let registry = FakeRegistry(list: .success(.init(devices: [me, other], olderClientSeenOn: nil)))
        registry.revoke = .failure(.unavailable)
        let viewModel = model(registry)
        var logOutCalled = false

        let loggedOut = await viewModel.logOutThisDevice(me) { logOutCalled = true }

        XCTAssertFalse(loggedOut)
        XCTAssertFalse(logOutCalled)
        XCTAssertEqual(viewModel.failure, .logOut)
    }
}
