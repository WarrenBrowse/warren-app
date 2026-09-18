//
//  WarrenForumActivityPanel.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  The state behind the forum activity panel: the one forum request tied to
//  an account, made only when the user opens the panel and never on a timer.
//

import Foundation
import WarrenRustRuntime

/// What the panel is showing right now.
public enum WarrenForumActivityState: Equatable, Sendable {
    case loading
    case rows([WarrenForumNotification])
    /// The class the shared crate named, never a value.
    case failed(reason: String)

    public var notifications: [WarrenForumNotification] {
        if case let .rows(rows) = self { return rows }
        return []
    }
}

/// The account-bound forum calls, as a seam: the production reader signs and
/// POSTs in Rust with the wallet seed, and the tests answer without a wallet
/// or a network.
public protocol WarrenForumActivityReading: Sendable {
    func list() async -> WarrenForumNotificationsResult
    func markSeen() async -> Bool
}

/// The production reader: loads the mnemonic, derives the seed, hands it to
/// the FFI and wipes it. The seed exists for exactly one call.
public struct WarrenForumActivityReader: WarrenForumActivityReading {
    public init() {}

    public func list() async -> WarrenForumNotificationsResult {
        await withSeed(failure: .failed(reason: "identity")) {
            WarrenForumActivityClient.notifications(seed: $0)
        }
    }

    public func markSeen() async -> Bool {
        await withSeed(failure: false) {
            WarrenForumActivityClient.markNotificationsSeen(seed: $0)
        }
    }

    private func withSeed<T: Sendable>(
        failure: T,
        _ body: @escaping @Sendable (Data) -> T
    ) async -> T {
        await Task.detached(priority: .userInitiated) {
            guard let mnemonic = try? WarrenWalletKeychain.loadSecure(),
                let wallet = try? WarrenWallet.fromMnemonic(mnemonic)
            else {
                return failure
            }
            // The wallet zeroes its own seed on `forgetSecret`, and its deinit
            // does the same, so the 32 bytes live only as long as the call.
            defer { wallet.forgetSecret() }
            return body(wallet.seed)
        }.value
    }
}

/// The panel's state machine.
///
/// Opening the panel marks the list seen, which is what the forum's own bell
/// does there. The badge follows the observation immediately rather than
/// waiting for the digest to catch up, which is up to a server refresh plus a
/// client poll behind.
@MainActor
public final class WarrenForumActivityViewModel: ObservableObject {
    @Published public private(set) var state: WarrenForumActivityState = .loading

    private let reader: any WarrenForumActivityReading
    private let observe: (Int) -> Void

    /// - Parameter observe: what the panel just proved about the unread
    ///   count, handed to the activity monitor.
    public init(
        reader: any WarrenForumActivityReading = WarrenForumActivityReader(),
        observe: @escaping (Int) -> Void = { _ in }
    ) {
        self.reader = reader
        self.observe = observe
    }

    /// Reads the panel, then tells the forum the list has been seen.
    ///
    /// The mark-seen runs after the read and its failure is not surfaced: the
    /// write is idempotent and monotonic on the server, so a failure only
    /// means the next open marks again, and saying so would be an error about
    /// something the user never asked for.
    public func load() async {
        state = .loading
        let result = await reader.list()
        switch result {
        case let .ok(rows):
            state = .rows(rows.sorted { $0.createdAt > $1.createdAt })
            // What the reader is looking at is read, whatever the digest still
            // says.
            observe(0)
            _ = await reader.markSeen()
        case let .failed(reason):
            // A failed read proves nothing about the count, so the badge is
            // left exactly as it stands.
            state = .failed(reason: reason)
        }
    }
}
