//
//  WarrenAnnouncements.swift
//  WarrenRustRuntime
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  Swift facade over the `warren_announcements_ffi` Rust exports
//  (`warren-ios/src/warren_announcements_ffi.rs`). The Rust side verifies
//  the Ed25519 signature of the fetched `GET /v1/announcements` envelope
//  against the pinned server key, refuses an envelope that would move the
//  generation backwards, and applies the signed expiry, each announcement's
//  own TTL, the client-version range and the call-to-action URL check. No
//  crypto and no filtering rule in Swift.
//
//  An announcement is operator-authored text rendered verbatim next to a
//  clickable link, which is why the verification is where it is: what
//  reaches this side is already display-ready, and is rendered as plain
//  text, never as markup.
//

import Foundation
import WarrenRustRuntimeProxy

/// Severity of an announcement, driving the banner's indicator colour.
public enum WarrenAnnouncementLevel: String, Equatable, Sendable {
    case info
    case warning
    case error

    /// Maps a wire token, defaulting to [info]: a level this build does not
    /// know must not withhold the operator's words.
    static func of(_ token: String) -> WarrenAnnouncementLevel {
        WarrenAnnouncementLevel(rawValue: token) ?? .info
    }
}

/// The call to action of an announcement, present only when Rust judged the
/// URL safe to render as a link.
public struct WarrenAnnouncementCta: Equatable, Sendable {
    /// Button caption, the operator's words, rendered as plain text.
    public let label: String
    /// Destination, `https` and already checked on the Rust side.
    public let url: URL

    public init(label: String, url: URL) {
        self.label = label
        self.url = url
    }
}

/// One launch announcement, ready to display.
public struct WarrenAnnouncement: Equatable, Sendable, Identifiable {
    /// Server-assigned id. The dismissal is persisted under it.
    public let id: String
    /// One-line title, rendered as plain text.
    public let headline: String
    /// Body text, rendered as plain text.
    public let body: String
    public let level: WarrenAnnouncementLevel
    public let cta: WarrenAnnouncementCta?
    /// The campaign to ask for this account's code under. Its presence IS the
    /// offer, so nothing has to reconcile a flag with an id.
    public let voucherCampaignID: String?
    /// The code drawn for THIS account, filled in by the second,
    /// wallet-signed lookup. `nil` both for an account outside the cohort and
    /// while the lookup has not answered: either way the card shows the
    /// operator's text without a code block.
    public var voucherCode: String?

    public init(
        id: String,
        headline: String,
        body: String,
        level: WarrenAnnouncementLevel,
        cta: WarrenAnnouncementCta? = nil,
        voucherCampaignID: String? = nil,
        voucherCode: String? = nil
    ) {
        self.id = id
        self.headline = headline
        self.body = body
        self.level = level
        self.cta = cta
        self.voucherCampaignID = voucherCampaignID
        self.voucherCode = voucherCode
    }
}

/// What one verified envelope hands over.
public struct WarrenVerifiedAnnouncements: Equatable, Sendable {
    /// The announcements to display, in publication order.
    public let announcements: [WarrenAnnouncement]
    /// The signed envelope expiry. The holder stops displaying this set at
    /// that instant without a further fetch, which is what takes a card down
    /// on a client whose refresh is being blocked.
    public let activeUntil: Date

    public init(announcements: [WarrenAnnouncement], activeUntil: Date) {
        self.announcements = announcements
        self.activeUntil = activeUntil
    }
}

public enum WarrenAnnouncementsVerifier {
    /// Verifies `envelope` (raw bytes of the fetched `GET /v1/announcements`
    /// body) and renders what must be displayed right now.
    ///
    /// Returns `nil` when the envelope does not verify, moves the generation
    /// backwards, or is unreadable. A `nil` is never a reason to take a card
    /// down: the caller keeps the set it holds until that set's `activeUntil`
    /// passes.
    public static func verify(envelope: Data, currentVersion: String) -> WarrenVerifiedAnnouncements? {
        let raw: UnsafeMutablePointer<CChar>? = envelope.withUnsafeBytes { buffer in
            // An empty body can carry a null base address; refuse it here
            // rather than handing the FFI a null pointer.
            guard let base = buffer.baseAddress else {
                return nil
            }
            return warren_announcements_verify(
                base.assumingMemoryBound(to: UInt8.self),
                UInt(buffer.count),
                currentVersion
            )
        }
        guard let raw else { return nil }
        defer { warren_announcements_free(raw) }
        return decode(String(cString: raw))
    }

    /// Decodes the JSON table the FFI returns. Pure, so the shape is exercised
    /// off the device.
    ///
    /// A row with no id or no headline is dropped and the rest of the set
    /// still shows: one unreadable row must not erase a live announcement. A
    /// call to action whose URL Foundation cannot parse is dropped the same
    /// way, without withholding the announcement, because the operator's text
    /// is not the unsafe part.
    static func decode(_ json: String) -> WarrenVerifiedAnnouncements? {
        guard let data = json.data(using: .utf8),
            let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
            let rows = object["announcements"] as? [[String: Any]],
            let activeUntil = object["active_until"] as? NSNumber
        else {
            return nil
        }
        return WarrenVerifiedAnnouncements(
            announcements: rows.compactMap(decodeRow),
            activeUntil: Date(timeIntervalSince1970: activeUntil.doubleValue)
        )
    }

    private static func decodeRow(_ row: [String: Any]) -> WarrenAnnouncement? {
        guard let id = row["id"] as? String, !id.isEmpty,
            let headline = row["headline"] as? String, !headline.isEmpty
        else {
            return nil
        }
        return WarrenAnnouncement(
            id: id,
            headline: headline,
            body: row["body"] as? String ?? "",
            level: WarrenAnnouncementLevel.of(row["level"] as? String ?? ""),
            cta: decodeCta(row["cta"] as? [String: Any]),
            voucherCampaignID: row["voucher_campaign_id"] as? String
        )
    }

    private static func decodeCta(_ row: [String: Any]?) -> WarrenAnnouncementCta? {
        guard let row,
            let label = row["label"] as? String, !label.isEmpty,
            let rawURL = row["url"] as? String,
            let url = URL(string: rawURL)
        else {
            return nil
        }
        return WarrenAnnouncementCta(label: label, url: url)
    }
}
