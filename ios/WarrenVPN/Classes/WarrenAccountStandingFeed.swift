//
//  WarrenAccountStandingFeed.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  The wallet's port-forward abuse standing (warren-core doc 105 §5.4): what
//  this installation last heard, the foreground poll that keeps it current,
//  and how the app words it. The Android twins are
//  `WarrenAccountStandingPoller` and `AccountStandingText`.
//

import Foundation
import WarrenLogging
import WarrenRustRuntime

/// How the app words the standing, in the desktop's and Android's words: the
/// warning a strike raises, the day it was recorded, and the date a ban
/// lapses. Shared by the banner, the notification and the port-forwarding
/// screen, so the three always say the same thing.
enum WarrenAccountStandingText {
    /// Where a warning is contested: the address the reports page gives.
    static let abuseContact = "abuse@warrenbrowse.com"

    /// The page that states the strike rule and how to contest a warning or a
    /// suspension, the one the desktop and Android link.
    static let reportsURL = "https://warren.ro/signalements"

    /// A day as the reader writes it. A strike is recorded at day precision,
    /// as midnight UTC, and a ban lapses on a day too, so both are formatted
    /// in UTC: in local time a strike would move to the day before for
    /// everyone west of Greenwich.
    static func day(_ date: Date, locale: Locale = .current) -> String {
        let formatter = DateFormatter()
        formatter.locale = locale
        formatter.dateStyle = .long
        formatter.timeStyle = .none
        formatter.timeZone = TimeZone(identifier: "UTC")
        return formatter.string(from: date)
    }

    static func category(_ wire: String) -> String {
        switch wire {
        case "copyright": String(localized: "copyright", table: "Settings")
        case "malware_c2": String(localized: "malware", table: "Settings")
        case "spam": String(localized: "spam", table: "Settings")
        case "scanning": String(localized: "scanning or intrusion", table: "Settings")
        case "phishing": String(localized: "phishing", table: "Settings")
        case "csam": String(localized: "child sexual abuse material", table: "Settings")
        default: String(localized: "other", table: "Settings")
        }
    }

    /// "Warning 1 of 3: port N was closed on DAY after an abuse report (category)."
    static func warning(_ notice: WarrenStrikeNotice, locale: Locale = .current) -> String {
        let strike = notice.strike
        if notice.threshold <= 0 {
            return String(
                format: String(
                    localized: "Warning %1$ld: port %2$ld was closed on %3$@ after an abuse report (%4$@).",
                    table: "Settings"
                ),
                notice.ordinal, strike.port, day(strike.day, locale: locale), category(strike.category)
            )
        }
        return String(
            format: String(
                localized: "Warning %1$ld of %2$ld: port %3$ld was closed on %4$@ after an abuse report (%5$@).",
                table: "Settings"
            ),
            notice.ordinal, notice.threshold, strike.port, day(strike.day, locale: locale),
            category(strike.category)
        )
    }

    /// The reference a contest quotes.
    static func caseReference(_ strike: WarrenAccountStrike) -> String {
        String(format: String(localized: "Case reference: %@", table: "Settings"), strike.caseReference)
    }

    /// How to contest a warning.
    static func contest() -> String {
        String(
            format: String(localized: "To contest a warning, write to %@ quoting its case reference.", table: "Settings"),
            abuseContact
        )
    }

    /// The ban as one line: what for, and until when when that is known.
    static func ban(_ ban: WarrenAccountBan, locale: Locale = .current) -> String {
        let until = ban.lapsesAt.map { day($0, locale: locale) }
        switch (ban.portForwarding, until) {
        case let (true, until?):
            return String(
                format: String(localized: "Access suspended for port-forwarding abuse until %@.", table: "Settings"),
                until
            )
        case (true, nil):
            return String(localized: "Access suspended for port-forwarding abuse.", table: "Settings")
        case let (false, until?):
            return String(format: String(localized: "Access suspended until %@.", table: "Settings"), until)
        case (false, nil):
            return String(localized: "Access suspended.", table: "Settings")
        }
    }
}

/// Holds the standing and polls it while the app is in the foreground, on the
/// ten minute cadence the token refresh and the desktop daemon use.
///
/// The poll is wallet-signed: it is the one request here tied to the account,
/// and it says nothing an issuance request does not already say. Each strike
/// Rust hands over is announced once, because Rust remembers across launches
/// which it already handed over.
final class WarrenAccountStandingFeed: @unchecked Sendable {
    static let checkInterval: TimeInterval = 10 * 60

    /// The wallet, the signed poll and the notification, injected so the whole
    /// feed can be driven without the keychain, the network or the system.
    struct Backend {
        /// Whether this device holds a wallet at all.
        var hasWallet: () -> Bool
        /// One signed poll, `nil` when it could not be made.
        var poll: () async -> WarrenStandingPoll?
        /// The wallet left: Rust forgets its standing and its ledger.
        var forget: () -> Void
        /// One system notification for one new strike.
        var announce: (WarrenStrikeNotice) -> Void
    }

    /// The feed the port-forwarding screen reads, set by the app delegate.
    nonisolated(unsafe) static weak var current: WarrenAccountStandingFeed?

    /// The standing moved: the banner and the screen have to be re-rendered.
    var didChange: (() -> Void)?

    private let logger = Logger(label: "WarrenAccountStandingFeed")
    private let backend: Backend
    private let lock = NSLock()
    private var held: WarrenAccountStanding?
    private var lastAnswer: Date?
    private var timer: Timer?

    init(backend: Backend) {
        self.backend = backend
    }

    /// What this installation last heard, `nil` while nothing is known.
    var standing: WarrenAccountStanding? {
        lock.withLock { held }
    }

    /// The app came to the foreground: poll at once when due, then keep
    /// polling while it stays there.
    func startPolling() {
        Task { [weak self] in await self?.refreshIfDue() }
        DispatchQueue.main.async { [weak self] in
            guard let self, timer == nil else { return }
            timer = Timer.scheduledTimer(withTimeInterval: Self.checkInterval, repeats: true) {
                [weak self] _ in
                Task { await self?.refreshIfDue() }
            }
        }
    }

    /// The app left the foreground. Nothing is polled from the background.
    func stopPolling() {
        DispatchQueue.main.async { [weak self] in
            self?.timer?.invalidate()
            self?.timer = nil
        }
    }

    func refreshIfDue(now: Date = Date()) async {
        let due = lock.withLock {
            warrenAnnouncementsFetchIsDue(lastFetch: lastAnswer, now: now, interval: Self.checkInterval)
        }
        guard due else { return }
        await refresh(now: now)
    }

    /// One poll, published.
    ///
    /// A failed poll keeps what is shown: the last answer still holds better
    /// than nothing, and the next tick asks again. A device that holds no
    /// wallet any more forgets the one that left, standing and ledger both.
    func refresh(now: Date = Date()) async {
        guard backend.hasWallet() else {
            let had = lock.withLock { () -> Bool in
                defer {
                    held = nil
                    lastAnswer = nil
                }
                return held != nil
            }
            backend.forget()
            if had { didChange?() }
            return
        }
        guard let poll = await backend.poll(), poll.ok else {
            logger.debug("Account standing poll failed; keeping what is shown")
            return
        }
        let changed = lock.withLock { () -> Bool in
            lastAnswer = now
            defer { held = poll.standing }
            return held != poll.standing
        }
        poll.newStrikes.forEach(backend.announce)
        if changed { didChange?() }
    }
}
