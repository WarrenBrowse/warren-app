//
//  MullvadApi.swift
//  WarrenVPNUITests
//
//  Created by Emils on 31/01/2024.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import Foundation
import WarrenLogging
import WarrenRustRuntime

struct MullvadApiError: Error {
    let description: String
}

private struct NewAccountResponse: Decodable { let number: String }
private struct AccountResponse: Decodable { let expiry: Date }

/// - Warning: Do not change the `apiAddress` or the `hostname` after the time `MullvadApi.init` has been invoked.
class MullvadApi {
    private let context: SwiftApiContext

    private static let logger = Logger(label: "MullvadApi")

    init(apiAddress: String, hostname: String) throws {
        Self.logger.debug("Initializing MullvadApi with address: \(apiAddress), hostname: \(hostname)")
        let directRaw = convert_builtin_access_method_setting(
            UUID().uuidString, "Direct", true, UInt8(KindDirect.rawValue), nil
        )
        let settingsWrapper = init_access_method_settings_wrapper(directRaw, nil, 0)
        context = mullvad_api_init_new(hostname, apiAddress, settingsWrapper, nil, nil)
    }

    func createAccount() throws -> String {
        let response = try makeRequest { cookie, strategy in
            mullvad_ios_create_account(context, cookie, strategy)
        }
        let data = try requireBody(response)
        return try JSONDecoder().decode(NewAccountResponse.self, from: data).number
    }

    func delete(account: String) throws {
        _ = try makeRequest { cookie, strategy in
            mullvad_ios_delete_account(context, cookie, strategy, account)
        }
    }

    func getExpiry(forAccount: String) throws -> UInt64 {
        let response = try makeRequest { cookie, strategy in
            mullvad_ios_get_account(context, cookie, strategy, forAccount)
        }
        let data = try requireBody(response)
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        let decoded = try decoder.decode(AccountResponse.self, from: data)
        return UInt64(decoded.expiry.timeIntervalSince1970)
    }

    private func requireBody(_ response: MullvadApiResponse) throws -> Data {
        guard response.success, let data = response.body else {
            throw MullvadApiError(description: response.errorDescription ?? "Request failed")
        }
        return data
    }

    @discardableResult
    private func makeRequest(
        _ call: (UnsafeMutableRawPointer, SwiftRetryStrategy) -> SwiftCancelHandle
    ) throws -> MullvadApiResponse {
        let semaphore = DispatchSemaphore(value: 0)
        nonisolated(unsafe) var apiResponse: MullvadApiResponse?

        let completion = MullvadApiCompletion { response in
            apiResponse = response
            semaphore.signal()
        }
        let cookie = Unmanaged.passRetained(completion).toOpaque()
        let strategy = mullvad_api_retry_strategy_constant(3, 1)
        var handle = call(cookie, strategy)
        semaphore.wait()
        mullvad_api_cancel_task_drop(&handle)

        guard let response = apiResponse else {
            throw MullvadApiError(description: "No response received")
        }
        return response
    }
}
