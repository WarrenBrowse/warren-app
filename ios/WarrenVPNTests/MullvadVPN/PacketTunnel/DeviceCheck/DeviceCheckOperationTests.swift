//
//  DeviceCheckOperationTests.swift
//  MullvadVPNTests
//
//  Created by pronebird on 30/05/2023.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import WarrenREST
import WarrenSettings
import WarrenTypes
import Operations
import PacketTunnelCore
import XCTest

@testable import WarrenMockData

class DeviceCheckOperationTests: XCTestCase {
    private let operationQueue = AsyncOperationQueue()
    private let dispatchQueue = DispatchQueue(label: "TestQueue")

    func testShouldReportExpiredAccount() async {
        let expect = expectation(description: "Wait for operation to complete")

        let remoteService = MockRemoteService(
            getAccount: { _ in
                Account.mock(expiry: .distantPast)
            }
        )
        let deviceStateAccessor = MockDeviceStateAccessor.mockLoggedIn()

        startDeviceCheck(remoteService: remoteService, deviceStateAccessor: deviceStateAccessor) { result in
            let deviceCheck = result.value

            XCTAssertNotNil(deviceCheck)
            XCTAssertTrue(deviceCheck?.accountVerdict.isExpired ?? false)

            expect.fulfill()
        }

        await fulfillment(of: [expect], timeout: .UnitTest.timeout)
    }

    func testShouldReportInvalidAccount() async {
        let expect = expectation(description: "Wait for operation to complete")

        let remoteService = MockRemoteService(
            getAccount: { _ in
                throw REST.Error.unhandledResponse(404, REST.ServerErrorResponse(code: .invalidAccount))
            }
        )
        let deviceStateAccessor = MockDeviceStateAccessor.mockLoggedIn()

        startDeviceCheck(remoteService: remoteService, deviceStateAccessor: deviceStateAccessor) { result in
            let deviceCheck = result.value

            XCTAssertNotNil(deviceCheck)
            XCTAssert(deviceCheck?.accountVerdict == .invalid)

            expect.fulfill()
        }

        await fulfillment(of: [expect], timeout: .UnitTest.timeout)
    }

    func testShouldReportRevokedDevice() async {
        let expect = expectation(description: "Wait for operation to complete")

        let remoteService = MockRemoteService(
            getDevice: { _, _ in
                throw REST.Error.unhandledResponse(404, REST.ServerErrorResponse(code: .deviceNotFound))
            }
        )
        let deviceStateAccessor = MockDeviceStateAccessor.mockLoggedIn()

        startDeviceCheck(remoteService: remoteService, deviceStateAccessor: deviceStateAccessor) { result in
            let deviceCheck = result.value

            XCTAssertNotNil(deviceCheck)
            XCTAssert(deviceCheck?.deviceVerdict == .revoked)

            expect.fulfill()
        }

        await fulfillment(of: [expect], timeout: .UnitTest.timeout)
    }

    func testShouldReportActiveAccountAndDevice() async {
        let expect = expectation(description: "Wait for operation to complete")

        let remoteService = MockRemoteService()
        let deviceStateAccessor = MockDeviceStateAccessor.mockLoggedIn()

        startDeviceCheck(remoteService: remoteService, deviceStateAccessor: deviceStateAccessor) { result in
            let deviceCheck = result.value

            XCTAssertNotNil(deviceCheck)
            XCTAssert(deviceCheck?.deviceVerdict == .active)

            expect.fulfill()
        }

        await fulfillment(of: [expect], timeout: .UnitTest.timeout)
    }

    private func startDeviceCheck(
        remoteService: DeviceCheckRemoteServiceProtocol,
        deviceStateAccessor: DeviceStateAccessorProtocol,
        completion: @escaping (Result<DeviceCheck, Error>) -> Void
    ) {
        let operation = DeviceCheckOperation(
            dispatchQueue: dispatchQueue,
            remoteSevice: remoteService,
            deviceStateAccessor: deviceStateAccessor,
            completionHandler: completion
        )

        operationQueue.addOperation(operation)
    }
}

/// Mock implementation of a remote service used by `DeviceCheckOperation` to reach the API.
private class MockRemoteService: DeviceCheckRemoteServiceProtocol, @unchecked Sendable {
    typealias AccountDataHandler = (_ accountNumber: String) throws -> Account
    typealias DeviceDataHandler = (_ accountNumber: String, _ deviceIdentifier: String) throws -> Device

    private let getAccountDataHandler: AccountDataHandler?
    private let getDeviceDataHandler: DeviceDataHandler?

    init(
        getAccount: AccountDataHandler? = nil,
        getDevice: DeviceDataHandler? = nil
    ) {
        getAccountDataHandler = getAccount
        getDeviceDataHandler = getDevice
    }

    func getAccountData(
        accountNumber: String,
        completion: @escaping @Sendable (Result<Account, Error>) -> Void
    ) -> Cancellable {
        DispatchQueue.main.async { [self] in
            let result: Result<Account, Error> = Result {
                if let getAccountDataHandler {
                    return try getAccountDataHandler(accountNumber)
                } else {
                    return Account.mock()
                }
            }
            completion(result)
        }
        return AnyCancellable()
    }

    func getDevice(
        accountNumber: String,
        identifier: String,
        completion: @escaping @Sendable (Result<Device, Error>) -> Void
    ) -> Cancellable {
        DispatchQueue.main.async { [self] in
            let result: Result<Device, Error> = Result {
                if let getDeviceDataHandler {
                    return try getDeviceDataHandler(accountNumber, identifier)
                } else {
                    return Device.mock(publicKey: WireGuard.PrivateKey().publicKey)
                }
            }

            completion(result)
        }

        return AnyCancellable()
    }
}

/// Mock implementation of device state accessor used by `DeviceCheckOperation` to access the storage holding device
/// state.
private class MockDeviceStateAccessor: DeviceStateAccessorProtocol {
    private var state: DeviceState
    private let stateLock = NSLock()

    init(initialState: DeviceState) {
        state = initialState
    }

    func read() throws -> DeviceState {
        stateLock.lock()
        defer { stateLock.unlock() }
        return state
    }

    func write(_ deviceState: DeviceState) throws {
        stateLock.lock()
        defer { stateLock.unlock() }
        state = deviceState
    }
}

extension MockDeviceStateAccessor {
    static func mockLoggedIn() -> MockDeviceStateAccessor {
        MockDeviceStateAccessor(
            initialState: .loggedIn(
                StoredAccountData.mock(),
                StoredDeviceData.mock()
            ))
    }
}

private extension StoredAccountData {
    static func mock() -> StoredAccountData {
        StoredAccountData(
            identifier: "account-id",
            number: "account-number",
            expiry: .distantFuture
        )
    }
}

private extension StoredDeviceData {
    static func mock() -> StoredDeviceData {
        StoredDeviceData(
            creationDate: Date(),
            identifier: "device-id",
            name: "device-name",
            hijackDNS: false,
            ipv4Address: IPAddressRange(from: "127.0.0.1/32")!,
            ipv6Address: IPAddressRange(from: "::ff/64")!,
            wgKeyData: StoredWgKeyData(creationDate: Date(), privateKey: WireGuard.PrivateKey())
        )
    }
}

private extension AccountVerdict {
    /// Returns `true` if account verdict is `.expired`.
    var isExpired: Bool {
        if case .expired = self {
            return true
        }
        return false
    }
}
