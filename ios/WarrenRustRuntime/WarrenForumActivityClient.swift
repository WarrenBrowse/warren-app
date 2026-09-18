//
//  WarrenForumActivityClient.swift
//  WarrenRustRuntime
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  The community forum's activity surface: the broadcast digest that raises
//  the header badge, and the caller's own notification panel behind it.
//
//  The digest is one anonymous document, identical for every client, so
//  fetching it says nothing about the user. The panel read is the one forum
//  request tied to an account, and it is made only when the user opens the
//  panel. Both are signed, verified and sent in Rust (`warren_forum_ffi`);
//  this facade only decodes the envelopes the shared crate defines.
//

import Foundation

/// Why a digest fetch ended, which is what the caller sizes its next delay
/// on. The tokens are the shared crate's (`warren_forum::digest::Refresh`).
public enum WarrenForumDigestFetch: String, Equatable, Sendable {
    /// A fresh document verified and is held.
    case ok
    /// The server confirmed the held document is current.
    case notModified = "not-modified"
    /// Reached the server but did not accept the answer.
    case rejected
    /// Never reached the server: retry early.
    case transport

    /// Whether the fetch never reached the server, the only class that earns
    /// the fast retry.
    public var isUnreachable: Bool { self == .transport }
}

/// One conditional fetch: the verified counts, `nil` while no fresh document
/// is held, and why the fetch ended.
public struct WarrenForumDigest: Equatable, Sendable {
    public let counts: String?
    public let fetch: WarrenForumDigestFetch

    public init(counts: String?, fetch: WarrenForumDigestFetch) {
        self.counts = counts
        self.fetch = fetch
    }
}

/// What happened on the forum, as the shared crate classed it. An unknown
/// token is `other` rather than a dropped row: a Discourse upgrade adding a
/// type must not make a notification vanish.
public enum WarrenForumNotificationKind: String, Equatable, Sendable {
    case mentioned
    case replied
    case quoted
    case liked
    case privateMessage = "private_message"
    case posted
    case linked
    case grantedBadge = "granted_badge"
    case watchingFirstPost = "watching_first_post"
    case announcement
    case other
}

/// One row of the activity panel. Every field was validated in Rust rather
/// than cast, so nothing here can carry arbitrary markup or a link out of the
/// forum.
public struct WarrenForumNotification: Equatable, Sendable, Identifiable {
    public let id: Int64
    public let kind: WarrenForumNotificationKind
    /// Unread by the forum's own rule, so an unread row is one the badge
    /// counted.
    public let unread: Bool
    public let createdAt: Date
    public let title: String?
    public let actor: String?
    public let excerpt: String?
    /// Forum-relative path the row opens, absent when it points at nothing
    /// openable.
    public let path: String?

    public init(
        id: Int64,
        kind: WarrenForumNotificationKind,
        unread: Bool,
        createdAt: Date,
        title: String?,
        actor: String?,
        excerpt: String?,
        path: String?
    ) {
        self.id = id
        self.kind = kind
        self.unread = unread
        self.createdAt = createdAt
        self.title = title
        self.actor = actor
        self.excerpt = excerpt
        self.path = path
    }
}

/// Outcome of one panel read.
public enum WarrenForumNotificationsResult: Equatable, Sendable {
    case ok([WarrenForumNotification])
    /// Not attempted, or failed; the reason is a fixed class for the log,
    /// never a value.
    case failed(reason: String)
}

/// The forum activity calls, as Swift sees them.
public enum WarrenForumActivityClient {
    private static let seedByteCount = 32

    /// One conditional fetch of the broadcast activity digest. Blocking (the
    /// GET runs in Rust): call off the main thread.
    ///
    /// `nil` when the FFI produced no envelope at all, which says nothing
    /// about the document being held: the caller must then leave the badge
    /// alone rather than read the silence as "no activity".
    public static func digestFetch() -> WarrenForumDigest? {
        guard let raw = warren_forum_digest_fetch() else { return nil }
        defer { warren_wallet_free_mnemonic(raw) }
        return digest(fromEnvelope: String(cString: raw))
    }

    /// The caller's own notifications. Blocking (signed and POSTed in Rust):
    /// call off the main thread. The seed is never logged on either side.
    public static func notifications(seed: Data) -> WarrenForumNotificationsResult {
        guard seed.count == seedByteCount else { return .failed(reason: "build") }
        let raw = seed.withUnsafeBytes { buffer -> UnsafeMutablePointer<CChar>? in
            guard let base = buffer.bindMemory(to: UInt8.self).baseAddress else { return nil }
            return warren_forum_notifications(base)
        }
        guard let raw else { return .failed(reason: "runtime") }
        defer { warren_wallet_free_mnemonic(raw) }
        return notifications(fromEnvelope: String(cString: raw))
    }

    /// Marks the caller's list seen, what opening the panel does. Blocking;
    /// returns whether the provider took the write. A failure only means the
    /// next open marks again: the write is idempotent and monotonic there.
    @discardableResult
    public static func markNotificationsSeen(seed: Data) -> Bool {
        guard seed.count == seedByteCount else { return false }
        let raw = seed.withUnsafeBytes { buffer -> UnsafeMutablePointer<CChar>? in
            guard let base = buffer.bindMemory(to: UInt8.self).baseAddress else { return nil }
            return warren_forum_notifications_seen(base)
        }
        guard let raw else { return false }
        defer { warren_wallet_free_mnemonic(raw) }
        return seen(fromEnvelope: String(cString: raw))
    }

    // MARK: - Envelope decoding (pure, tested off-device)

    /// `{"counts":"03f"|null,"fetch":"ok"}`. An envelope that cannot be read
    /// is a fetch that never reached the server, which is the class that
    /// retries soonest and holds no document.
    static func digest(fromEnvelope envelope: String?) -> WarrenForumDigest {
        guard let object = jsonObject(envelope) else {
            return WarrenForumDigest(counts: nil, fetch: .transport)
        }
        let fetch = (object["fetch"] as? String).flatMap(WarrenForumDigestFetch.init(rawValue:))
        return WarrenForumDigest(counts: object["counts"] as? String, fetch: fetch ?? .transport)
    }

    /// `{"ok":true,"notifications":[..]}` or the classed failure. A row the
    /// decoder cannot read is dropped rather than rendered, the rule the Rust
    /// parser already applies to the provider's own answer.
    static func notifications(fromEnvelope envelope: String?) -> WarrenForumNotificationsResult {
        guard let object = jsonObject(envelope) else { return .failed(reason: "unknown") }
        guard object["ok"] as? Bool == true else {
            return .failed(reason: object["reason"] as? String ?? "unknown")
        }
        let rows = (object["notifications"] as? [[String: Any]] ?? []).compactMap(notification(from:))
        return .ok(rows)
    }

    /// `{"ok":true}` and nothing else is a write the provider took.
    static func seen(fromEnvelope envelope: String?) -> Bool {
        jsonObject(envelope)?["ok"] as? Bool == true
    }

    private static func notification(from row: [String: Any]) -> WarrenForumNotification? {
        guard let id = (row["id"] as? NSNumber)?.int64Value,
            let createdAt = (row["created_at"] as? NSNumber)?.doubleValue
        else {
            return nil
        }
        let kind = (row["kind"] as? String).flatMap(WarrenForumNotificationKind.init(rawValue:))
        return WarrenForumNotification(
            id: id,
            kind: kind ?? .other,
            unread: row["unread"] as? Bool == true,
            createdAt: Date(timeIntervalSince1970: createdAt),
            title: row["title"] as? String,
            actor: row["actor"] as? String,
            excerpt: row["excerpt"] as? String,
            path: row["path"] as? String
        )
    }

    private static func jsonObject(_ envelope: String?) -> [String: Any]? {
        guard let envelope,
            let data = envelope.data(using: .utf8)
        else {
            return nil
        }
        return try? JSONSerialization.jsonObject(with: data) as? [String: Any]
    }
}
