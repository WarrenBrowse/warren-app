//
//  MullvadApiRequestFactory.swift
//  MullvadVPN
//
//  Created by Jon Petersson on 2025-02-07.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import WarrenRustRuntime
import WarrenTypes

public struct MullvadApiRequestFactory: Sendable {
    public let apiContext: MullvadApiContext
    private let encoder: JSONEncoder

    public init(apiContext: MullvadApiContext, encoder: JSONEncoder) {
        self.apiContext = apiContext
        self.encoder = encoder
    }

    public func makeRequest(_ request: APIRequest) -> REST.MullvadApiRequestHandler {
        { completion in
            let completionPointer = MullvadApiCompletion { apiResponse in
                try? completion?(apiResponse)
            }

            let rawCompletionPointer = Unmanaged.passRetained(completionPointer).toOpaque()

            switch request {
            case let .getAddressList(retryStrategy):
                return MullvadApiCancellable(
                    handle: mullvad_ios_get_addresses(
                        apiContext.context,
                        rawCompletionPointer,
                        retryStrategy.toRustStrategy()
                    ))

            case let .getRelayList(retryStrategy, etag: etag):
                return MullvadApiCancellable(
                    handle: mullvad_ios_get_relays(
                        apiContext.context,
                        rawCompletionPointer,
                        retryStrategy.toRustStrategy(),
                        etag
                    ))
            case let .sendProblemReport(retryStrategy, problemReportRequest):
                let rustRequest = RustProblemReportRequest(from: problemReportRequest)
                return MullvadApiCancellable(
                    handle: mullvad_ios_send_problem_report(
                        apiContext.context,
                        rawCompletionPointer,
                        retryStrategy.toRustStrategy(),
                        rustRequest.toRust()
                    ))
            case let .getAccount(retryStrategy, accountNumber: accountNumber):
                return MullvadApiCancellable(
                    handle: mullvad_ios_get_account(
                        apiContext.context,
                        rawCompletionPointer,
                        retryStrategy.toRustStrategy(),
                        accountNumber
                    ))
            case let .createAccount(retryStrategy):
                return MullvadApiCancellable(
                    handle: mullvad_ios_create_account(
                        apiContext.context,
                        rawCompletionPointer,
                        retryStrategy.toRustStrategy()
                    ))
            case let .deleteAccount(retryStrategy, accountNumber: accountNumber):
                return MullvadApiCancellable(
                    handle: mullvad_ios_delete_account(
                        apiContext.context,
                        rawCompletionPointer,
                        retryStrategy.toRustStrategy(),
                        accountNumber
                    ))

            case let .checkApiAvailability(retryStrategy, accessMethod):
                return MullvadApiCancellable(
                    handle: mullvad_ios_api_addrs_available(
                        apiContext.context,
                        rawCompletionPointer,
                        retryStrategy.toRustStrategy(),
                        convertAccessMethod(accessMethod: accessMethod)
                    ))
            case let .initStorekitPayment(
                retryStrategy: retryStrategy,
                accountNumber: accountNumber
            ):
                return MullvadApiCancellable(
                    handle: mullvad_ios_init_storekit_payment(
                        apiContext.context,
                        rawCompletionPointer,
                        retryStrategy.toRustStrategy(),
                        accountNumber
                    ))
            case let .checkStorekitPayment(
                retryStrategy: retryStrategy,
                transaction: transaction
            ):
                let body = try encoder.encode(transaction)
                return MullvadApiCancellable(
                    handle: mullvad_ios_check_storekit_payment(
                        apiContext.context,
                        rawCompletionPointer,
                        retryStrategy.toRustStrategy(),
                        body.map { $0 },
                        UInt(body.count)
                    ))
            }
        }
    }
}

extension REST {
    public typealias MullvadApiRequestHandler = (((MullvadApiResponse) throws -> Void)?) throws -> MullvadApiCancellable
}
