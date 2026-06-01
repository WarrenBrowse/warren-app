//
//  DeviceCheckOperation.swift
//  PacketTunnel
//
//  Created by pronebird on 20/04/2023.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import Foundation
import WarrenREST
import WarrenSettings
import WarrenTypes
import Operations
import PacketTunnelCore

/**
 An operation that is responsible for performing account and device diagnostics from within packet tunnel process.
 */
final class DeviceCheckOperation: ResultOperation<DeviceCheck>, @unchecked Sendable {
    private let remoteService: DeviceCheckRemoteServiceProtocol
    private let deviceStateAccessor: DeviceStateAccessorProtocol

    private var tasks: [Cancellable] = []

    init(
        dispatchQueue: DispatchQueue,
        remoteSevice: DeviceCheckRemoteServiceProtocol,
        deviceStateAccessor: DeviceStateAccessorProtocol,
        completionHandler: CompletionHandler? = nil
    ) {
        self.remoteService = remoteSevice
        self.deviceStateAccessor = deviceStateAccessor

        super.init(dispatchQueue: dispatchQueue, completionQueue: dispatchQueue, completionHandler: completionHandler)
    }

    override func main() {
        startFlow { result in
            self.finish(result: result)
        }
    }

    override func operationDidCancel() {
        tasks.forEach { $0.cancel() }
    }

    // MARK: - Flow

    /**
     Begins the flow by fetching device state and then fetching account and device data. Calls `didReceiveData()` with
     the received data when done.
     */
    private func startFlow(completion: @escaping @Sendable (Result<DeviceCheck, Error>) -> Void) {
        do {
            guard case let .loggedIn(accountData, deviceData) = try deviceStateAccessor.read() else {
                throw DeviceCheckError.invalidDeviceState
            }

            fetchData(
                accountNumber: accountData.number,
                deviceIdentifier: deviceData.identifier
            ) { [self] accountResult, deviceResult in
                didReceiveData(accountResult: accountResult, deviceResult: deviceResult, completion: completion)
            }
        } catch {
            completion(.failure(error))
        }
    }

    /// Handles received data results and produces the account and device verdicts.
    private func didReceiveData(
        accountResult: Result<Account, Error>,
        deviceResult: Result<Device, Error>,
        completion: @escaping @Sendable (Result<DeviceCheck, Error>) -> Void
    ) {
        do {
            let accountVerdict = try accountVerdict(from: accountResult)
            let deviceVerdict = try deviceVerdict(from: deviceResult)

            completion(.success(DeviceCheck(accountVerdict: accountVerdict, deviceVerdict: deviceVerdict)))
        } catch {
            completion(.failure(error))
        }
    }

    // MARK: - Data fetch

    /// Fetch account and device data simultaneously, upon completion calls completion handler passing the results to
    /// it.
    private func fetchData(
        accountNumber: String, deviceIdentifier: String,
        completion: @escaping (Result<Account, Error>, Result<Device, Error>) -> Void
    ) {
        nonisolated(unsafe) var accountResult: Result<Account, Error> = .failure(OperationError.cancelled)
        nonisolated(unsafe) var deviceResult: Result<Device, Error> = .failure(OperationError.cancelled)

        let dispatchGroup = DispatchGroup()

        dispatchGroup.enter()
        let accountTask = remoteService.getAccountData(accountNumber: accountNumber) { result in
            accountResult = result
            dispatchGroup.leave()
        }

        dispatchGroup.enter()
        let deviceTask = remoteService.getDevice(accountNumber: accountNumber, identifier: deviceIdentifier) { result in
            deviceResult = result
            dispatchGroup.leave()
        }

        tasks.append(contentsOf: [accountTask, deviceTask])

        dispatchGroup.notify(queue: dispatchQueue) {
            completion(accountResult, deviceResult)
        }
    }

    // MARK: - Private helpers

    /// Converts account data result type into `AccountVerdict`.
    private func accountVerdict(from accountResult: Result<Account, Error>) throws -> AccountVerdict {
        do {
            let account = try accountResult.get()

            return account.expiry > Date() ? .active(account) : .expired(account)
        } catch let error as REST.Error where error.compareErrorCode(.invalidAccount) {
            return .invalid
        }
    }

    /// Converts device result type into `DeviceVerdict`.
    private func deviceVerdict(from deviceResult: Result<Device, Error>) throws -> DeviceVerdict {
        do {
            _ = try deviceResult.get()

            return .active
        } catch let error as REST.Error where error.compareErrorCode(.deviceNotFound) {
            return .revoked
        }
    }
}

/// An error used internally by `DeviceCheckOperation`.
public enum DeviceCheckError: LocalizedError, Equatable {
    /// Device is no longer logged in.
    case invalidDeviceState

    public var errorDescription: String? {
        switch self {
        case .invalidDeviceState:
            return "Cannot complete device check because device is no longer logged in."
        }
    }
}
