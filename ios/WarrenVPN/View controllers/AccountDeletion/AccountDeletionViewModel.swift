//
//  AccountDeletionViewModel.swift
//  MullvadVPN
//
//  Created by Mojgan on 2023-07-13.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import Foundation
import SwiftUI
import WarrenRustRuntime

protocol AccountDeletionBackEnd: Sendable {
    var accountNumber: String? { get }

    func deleteAccount(accountNumber: String) async throws
}

struct TunnelManagerAccountDeletionBackEnd: AccountDeletionBackEnd {
    let tunnelManager: TunnelManager

    var accountNumber: String? {
        tunnelManager.deviceState.accountData?.number
    }

    /// Warren account deletion: delete the subscription server-side
    /// (signed `DELETE /v1/account`), then wipe the local wallet. A 404
    /// means the wallet has no subscription bound, which is the desired
    /// end state, so we proceed to the local wipe. Any other server /
    /// transport failure aborts before the wallet is wiped, so the user
    /// does not lose their identity while the subscription record
    /// lingers.
    func deleteAccount(accountNumber: String) async throws {
        let walletInteractor = WarrenWalletInteractor()
        try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Void, Swift.Error>) in
            walletInteractor.deleteServerAccount { result in
                switch result {
                case .success:
                    continuation.resume()
                case let .failure(.account(.server(status, _))) where status == 404:
                    continuation.resume()
                case let .failure(error):
                    continuation.resume(throwing: error)
                }
            }
        }
        await MainActor.run {
            WarrenWalletLogout.perform(tunnelManager: tunnelManager)
        }
    }
}

struct MockAccountDeletionBackEnd: AccountDeletionBackEnd {
    let accountNumber: String?

    func deleteAccount(accountNumber: String) async throws {}
}

class AccountDeletionViewModel: ObservableObject {
    enum State {
        case initial
        case working
        case failure(Swift.Error)
    }

    enum Error: LocalizedError {
        case invalidInput

        var errorDescription: String? {
            switch self {
            case .invalidInput:
                return NSLocalizedString("Last four digits of the account number are incorrect", comment: "")
            }
        }
    }

    @Published var accountNumber: String
    @Published var enteredAccountNumberSuffix = ""
    @Published var state: State = .initial

    private let backEnd: AccountDeletionBackEnd

    var onConclusion: ((Bool) -> Void)?

    var tunnelManagerAccountNumber: String {
        backEnd.accountNumber ?? ""
    }

    var accountNumberSuffix: Substring {
        accountNumber.suffix(4)
    }

    init(tunnelManager: TunnelManager, onConclusion: ((Bool) -> Void)? = nil) {
        self.backEnd = TunnelManagerAccountDeletionBackEnd(tunnelManager: tunnelManager)
        self.accountNumber = tunnelManager.deviceState.accountData?.number.formattedAccountNumber ?? ""
        self.onConclusion = onConclusion
    }

    // for SwiftUI previews
    init(mockAccountNumber: String?) {
        self.backEnd = MockAccountDeletionBackEnd(accountNumber: mockAccountNumber)
        self.accountNumber = mockAccountNumber ?? ""
        self.onConclusion = nil
    }

    var messageText: LocalizedStringKey {
        var attributedAccountNumber: AttributedString {
            return
                (try? AttributedString(
                    markdown: "**\(accountNumber)**",
                    options: AttributedString.MarkdownParsingOptions(interpretedSyntax: .inlineOnlyPreservingWhitespace)
                )) ?? AttributedString(accountNumber)
        }
        return LocalizedStringKey("Are you sure you want to delete the account \(attributedAccountNumber)?")
    }

    var statusText: LocalizedStringKey? {
        switch state {
        case let .failure(error):
            LocalizedStringKey(error.localizedDescription)
        case .working:
            LocalizedStringKey("Deleting account...")
        default: nil
        }
    }

    var canDelete: Bool {
        !isWorking && enteredAccountNumberSuffix.count == 4 && accountNumberSuffix == enteredAccountNumberSuffix
    }

    var isWorking: Bool {
        switch state {
        case .working: true
        default: false
        }
    }

    func validate(input: String) -> Result<String, Error> {
        if let deviceAccountNumber = backEnd.accountNumber,
            let fourLastDigits = deviceAccountNumber.split(every: 4).last,
            fourLastDigits == input
        {
            return .success(deviceAccountNumber)
        } else {
            return .failure(Error.invalidInput)
        }
    }

    @MainActor func deleteButtonTapped() {
        switch validate(input: enteredAccountNumberSuffix) {
        case let .success(accountNumber):
            doDelete(accountNumber: accountNumber)
        case let .failure(error):
            state = .failure(error)
        }
    }

    func cancelButtonTapped() {
        self.onConclusion?(false)
    }

    @MainActor func doDelete(accountNumber: String) {
        state = .working
        Task { [weak self] in
            guard let self else { return }
            do {
                try await backEnd.deleteAccount(accountNumber: accountNumber)
                self.state = State.initial
                self.onConclusion?(true)
            } catch {
                self.state = State.failure(error)
            }
        }
    }
}
