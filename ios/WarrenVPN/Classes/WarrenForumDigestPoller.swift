//
//  WarrenForumDigestPoller.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  The foreground poll of the broadcast forum digest. No background task and
//  no push carries it: a background cadence would make the app a periodic
//  presence signal for a badge nobody is looking at, and the fetch on
//  foreground already catches up on whatever happened meanwhile
//  (docs/warren-forum-login.md).
//

import Foundation
import WarrenRustRuntime

/// When the next digest fetch runs, from what the last one brought back: the
/// daemon's `warren_forum_digest_updater` cadence, kept here because the app
/// owns the loop on this platform, exactly as Android's `ForumDigestCadence`
/// does.
///
/// A minute between checks: how long a reply takes to raise a badge, and how
/// long a badge cleared elsewhere takes to drop here. Bounded by design: the
/// request is a conditional GET on a document identical for every client, so
/// a quiet forum answers it with a 304. After a fetch that never reached the
/// server the retry doubles from 20 s up to 45 s, so a client that just
/// regained a network does not sit a full interval with a badge it can no
/// longer justify; a server that answered, whatever it said, clears the fast
/// retry.
public enum WarrenForumDigestCadence {
    public static let checkInterval: TimeInterval = 60
    public static let retryMin: TimeInterval = 20
    public static let retryMax: TimeInterval = 45

    /// The delay before the next fetch, and the retry state to carry over.
    public static func next(
        unreachable: Bool,
        retry: TimeInterval?
    ) -> (delay: TimeInterval, retry: TimeInterval?) {
        guard unreachable else { return (checkInterval, nil) }
        let armed = retry.map { min($0 * 2, retryMax) } ?? retryMin
        return (armed, armed)
    }
}

/// Whether the broadcast digest has a reader on this installation: the forum
/// notifications setting on, and a forum account, whose slot is what indexes
/// the digest. Without both, the fetch is a periodic handshake with the API
/// host for a number nobody displays.
///
/// The desktop daemon polls the digest unconditionally, beside its relay-list
/// and notices refreshes on the channel its own API traffic already uses;
/// here the loop is the app's own, so the gate costs nothing and is applied.
public func warrenForumDigestWanted(notificationsEnabled: Bool, hasAccount: Bool) -> Bool {
    notificationsEnabled && hasAccount
}

/// Runs the digest fetch on its cadence while the app is in the foreground and
/// something on this installation reads the result.
///
/// The fetch rides the tunnel when one is up, but the API host name still goes
/// through the system resolver, so a tunnel between states defers the fetch
/// rather than hanging it for 15 s: the same `WarrenForumPreflight` gate the
/// signed forum calls already pass.
@MainActor
public final class WarrenForumDigestPoller {
    /// The fetch itself, a seam so the loop is tested without a network.
    public typealias Fetch = @Sendable () -> WarrenForumDigest?

    private let fetch: Fetch
    private let preflight: @MainActor () -> Bool
    private let apply: @MainActor (String?) -> Void
    private var task: Task<Void, Never>?

    /// - Parameters:
    ///   - preflight: whether the resolver is usable right now.
    ///   - apply: hands the verified counts to the monitor.
    public init(
        fetch: @escaping Fetch = { WarrenForumActivityClient.digestFetch() },
        preflight: @escaping @MainActor () -> Bool,
        apply: @escaping @MainActor (String?) -> Void
    ) {
        self.fetch = fetch
        self.preflight = preflight
        self.apply = apply
    }

    deinit {
        task?.cancel()
    }

    /// Starts the loop, or restarts it so the next fetch happens at once.
    /// Called when the app comes to the foreground and when the feature is
    /// turned back on: a badge is never a minute late after either.
    public func start() {
        stop()
        task = Task { [weak self] in
            var retry: TimeInterval?
            while !Task.isCancelled {
                guard let self else { return }
                let (delay, next) = WarrenForumDigestCadence.next(
                    unreachable: await self.fetchOnce(), retry: retry)
                retry = next
                try? await Task.sleep(nanoseconds: UInt64(delay * 1_000_000_000))
            }
        }
    }

    public func stop() {
        task?.cancel()
        task = nil
    }

    /// One fetch fed into the monitor; returns whether it failed to reach the
    /// server, which is what the cadence keys on. A deferred fetch counts as
    /// unreachable: nothing was asked, so nothing was learned.
    func fetchOnce() async -> Bool {
        guard preflight() else { return true }
        let fetch = self.fetch
        // No envelope at all says nothing about the document Rust is holding,
        // so the badge is left exactly as it stands.
        guard let digest = await Task.detached(priority: .utility) { fetch() }.value else {
            return true
        }
        // Rust re-applies freshness on every read, so these counts are the
        // whole truth about what may be shown, whatever the fetch class was.
        apply(digest.counts)
        return digest.fetch.isUnreachable
    }
}
