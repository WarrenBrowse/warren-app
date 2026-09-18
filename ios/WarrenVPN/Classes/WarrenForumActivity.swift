//
//  WarrenForumActivity.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  How the broadcast forum activity digest becomes one user's badge, and what
//  the connect header's forum slot carries.
//
//  Both values behind it are minted server side and learned only from an
//  approved login: the handle is derived under a secret this app does not
//  hold, and the digest slot is drawn at random so the published document
//  stays anonymous. Keeping the slot beside the handle is what lets the app
//  read its own activity out of a document identical for every client,
//  without ever asking the server about this account.
//
//  The rules are pinned by `fixtures/client-rules/forum_activity.json` and
//  replayed by the desktop and Android readers of the same file.
//

import Foundation

/// Highest count one digest character can carry, above which it saturates.
public let warrenUnreadSaturated = 15

/// What the header's forum slot carries: the bell, the lifebuoy, or nothing.
public enum WarrenForumHeaderButton: String, Equatable, Sendable {
    case activity
    case community
    case none
}

/// What a notification about a rise says. One digest character per slot, so
/// the count stops climbing at its ceiling; saying "15" there would be a
/// number the user can check and find wrong, hence `moreThan` with the
/// highest count the app can measure exactly.
public enum WarrenForumActivityWording: Equatable, Sendable {
    case single
    case several(count: Int)
    case moreThan(count: Int)
}

/// The pure badge rules, shared with the two other clients.
public enum WarrenForumActivity {
    /// Unread count this installation's `slot` carries in `digest` (one
    /// lowercase hex character per slot).
    ///
    /// Zero whenever there is nothing to show: no fresh document, no slot
    /// yet, or a slot past what the server has published (the normal state of
    /// an account that registered since the last rebuild). Rust has already
    /// checked the signature and the freshness, so this only indexes.
    public static func unread(in digest: String?, slot: Int?) -> Int {
        guard let digest, let slot, slot >= 0, slot < digest.count else { return 0 }
        let character = digest[digest.index(digest.startIndex, offsetBy: slot)]
        return character.hexDigitValue ?? 0
    }

    /// The count as shown, saturating rather than growing the badge.
    public static func label(unread: Int) -> String {
        unread >= warrenUnreadSaturated ? "\(warrenUnreadSaturated)+" : "\(unread)"
    }

    /// Whether the app shows forum activity at all: the header bell, the
    /// local notification and the app icon badge alike. Off means off
    /// everywhere, and a wallet with no forum account has no activity to
    /// report.
    public static func showsActivity(hasAccount: Bool, enabled: Bool) -> Bool {
        hasAccount && enabled
    }

    /// Which button the header's forum slot shows.
    ///
    /// A wallet with no forum account gets a lifebuoy straight to the forum
    /// rather than an empty slot: the bell would be inert for them, but the
    /// forum is the one thing they might actually want, and it is where an
    /// account comes from. The setting still governs the whole slot, lifebuoy
    /// included.
    public static func headerButton(hasAccount: Bool, enabled: Bool) -> WarrenForumHeaderButton {
        guard enabled else { return .none }
        return hasAccount ? .activity : .community
    }

    public static func wording(unread: Int) -> WarrenForumActivityWording {
        if unread >= warrenUnreadSaturated {
            return .moreThan(count: warrenUnreadSaturated - 1)
        }
        return unread == 1 ? .single : .several(count: unread)
    }

    /// Whether a notification's age still reads better as "2 h ago" than as a
    /// date: past a week, "5 weeks ago" tells a reader less than the day it
    /// happened.
    public static func ageIsRelative(createdAt: Date, now: Date) -> Bool {
        now.timeIntervalSince(createdAt) < relativeAgeLimit
    }

    static let relativeAgeLimit: TimeInterval = 7 * 24 * 3600
}

/// Turns the broadcast digest into a badge and a local notification.
///
/// Everything it needs is already here: Rust has checked the document's
/// signature and freshness, and only this process knows which slot belongs to
/// this installation. So the whole feature costs no request, and the server is
/// never told that this account is watching.
///
/// Two rules do most of the work.
///
/// A notification is for activity that arrived while this run was watching.
/// The count already waiting when the app starts gets the badge but no
/// notification, otherwise every relaunch would re-announce the same rows.
///
/// An absent digest means unknown, never zero. The fetch drops the document
/// when it cannot refresh it, and reading that gap as "all read" would fire a
/// notification for what the user has already seen as soon as it came back.
///
/// Reading on the forum through any other channel needs no handling: it
/// advances the reader's own bookmark there, the next digest carries a lower
/// count, and the badge and the notification follow the same number.
@MainActor
public final class WarrenForumActivityMonitor {
    /// What the monitor drives. The app wires the badge, the local
    /// notification and the header to it; the tests observe it directly.
    /// Main-actor isolated like the monitor itself: every surface it drives
    /// is a view.
    @MainActor
    public protocol Delegate: AnyObject {
        /// A rise above what this run had accounted for, with the new count.
        func forumActivityDidRise(to unread: Int)

        /// Whether anything is waiting: drives the app icon badge and the
        /// notification's life.
        func forumActivityShowsIndicator(_ showing: Bool)

        /// Hands every surface the same number, so the bell cannot disagree.
        func forumActivityDidPublish(unread: Int)
    }

    public weak var delegate: Delegate?

    private var digest: String?
    private var slot: Int?
    private var enabled = true
    private var indicator = false

    /// Count this run has already accounted for, nil until a digest has
    /// actually been seen for the current slot: what separates "nothing new"
    /// from "nothing known yet".
    private var acknowledged: Int?

    /// What the app saw for itself, by reading the panel or by marking the
    /// list seen, and the digest that was current at the time. The digest is
    /// up to a server refresh plus a client poll behind, so without this the
    /// badge would sit on a stale number for minutes after the user acted.
    /// Held only until the digest is rebuilt: a changed document has either
    /// seen our write or carries something newer, and either way it is the
    /// better source. Pinning it to the document rather than to a clock is
    /// what makes that handover exact.
    private var observed: (unread: Int, digest: String?)?

    private var lastPublished: Int?

    public init(delegate: Delegate? = nil) {
        self.delegate = delegate
    }

    public func setDigest(_ digest: String?) {
        self.digest = digest
        refresh()
    }

    /// What a panel read or a mark-seen just proved, effective immediately.
    public func setObservedUnread(_ unread: Int) {
        observed = (unread, digest)
        // The user is looking at the panel or has just acted in it. Whatever
        // the number does here, it is not news to them.
        acknowledged = unread
        refresh()
    }

    public func setSlot(_ slot: Int?) {
        guard slot != self.slot else { return }
        // Another forum account, or none: its predecessor's count says
        // nothing about this one, and neither does what the app observed for
        // it. An observation kept past the account would drive the badge and
        // the notification for an account this installation no longer holds.
        self.slot = slot
        acknowledged = nil
        observed = nil
        refresh()
    }

    public func setEnabled(_ enabled: Bool) {
        guard enabled != self.enabled else { return }
        self.enabled = enabled
        refresh()
    }

    private func refresh() {
        if let held = observed, held.digest != digest {
            observed = nil
        }
        let unread = observed?.unread ?? WarrenForumActivity.unread(in: digest, slot: slot)

        showIndicator(enabled && unread > 0)
        publish(unread)

        // Keep the watermark: a missing digest or slot is a gap in what we
        // know, not a read.
        guard digest != nil, slot != nil else { return }

        let previous = acknowledged
        // Advanced even while the setting is off, so turning it back on does
        // not announce what happened in the meantime.
        acknowledged = unread

        guard let previous, unread > previous, enabled else { return }
        delegate?.forumActivityDidRise(to: unread)
    }

    private func publish(_ unread: Int) {
        guard unread != lastPublished else { return }
        lastPublished = unread
        delegate?.forumActivityDidPublish(unread: unread)
    }

    private func showIndicator(_ value: Bool) {
        guard value != indicator else { return }
        indicator = value
        delegate?.forumActivityShowsIndicator(value)
    }
}
