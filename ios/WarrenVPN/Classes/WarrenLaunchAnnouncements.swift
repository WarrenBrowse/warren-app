//
//  WarrenLaunchAnnouncements.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  The launch-announcement feed: what this installation currently holds, and
//  the foreground poll that keeps it current.
//
//  Everything that decides whether a card may be shown at all (the signature
//  against the pinned server key, the anti-rollback generation, the signed
//  expiry, each announcement's own TTL, the client-version range, the
//  call-to-action URL check) is enforced in Rust, which is why this holds only
//  what came back.
//

import Foundation
import WarrenLogging
import WarrenRustRuntime
import WarrenSettings

/// Whether an announcements fetch is due: never fetched, or the last one is
/// older than `interval`. A future `lastFetch` (clock rollback) counts as due
/// rather than silencing the poll for an arbitrary while. Same rule the
/// version gate uses for its daily manifest check.
func warrenAnnouncementsFetchIsDue(lastFetch: Date?, now: Date, interval: TimeInterval) -> Bool {
    guard let lastFetch else { return true }
    let elapsed = now.timeIntervalSince(lastFetch)
    return elapsed < 0 || elapsed >= interval
}

/// Where the reader's dismissals are kept. Behind a protocol so the card can
/// be exercised without UserDefaults.
protocol WarrenAnnouncementDismissing: AnyObject {
    /// Ids of the announcements the reader has put away, for good.
    var warrenDismissedAnnouncements: [String] { get set }
}

extension AppPreferences: WarrenAnnouncementDismissing {}

/// The verified set this installation holds, and the instant it stops being
/// displayable without a further fetch.
struct WarrenHeldAnnouncements: Equatable {
    var announcements: [WarrenAnnouncement] = []
    /// The signed envelope expiry Rust handed over. `distantPast` while
    /// nothing has ever verified, so an empty hold displays nothing.
    var activeUntil: Date = .distantPast

    static let empty = WarrenHeldAnnouncements()
}

/// Holds the verified announcements and polls for them while the app is in the
/// foreground.
///
/// No background task and no push carries this, for the reason the desktop
/// notice poll has: a background cadence would make the app a periodic beacon
/// for a card nobody is looking at, and the fetch on every activation already
/// catches up on whatever the operator published meanwhile.
///
/// The announcements ride a document that is byte-identical for every caller,
/// so the poll says nothing about the account. The code the offer carries
/// cannot ride that document, so it is drawn over a second, wallet-signed
/// call, and only for an announcement that actually carries a campaign: an
/// announcement with no offer never touches the wallet.
final class WarrenLaunchAnnouncements: @unchecked Sendable {
    /// Five minutes between checks while the app is visible: the delay an
    /// operator waits for a publication or a withdrawal to reach a running
    /// client. The desktop daemon and Android poll on the same interval.
    static let checkInterval: TimeInterval = 5 * 60

    /// The network and the wallet, injected so the whole poll can be driven
    /// without URLSession, the keychain or a signed request.
    struct Backend {
        /// One `GET /v1/announcements` against the compiled environment's API
        /// host, `nil` when the request never got an answer.
        var fetch: (URL) async -> Data?
        /// Verification, which is where every display rule lives.
        var verify: (Data, String) -> WarrenVerifiedAnnouncements?
        /// This account's code for `campaignID`, `nil` both for an account
        /// outside the cohort and for a lookup that failed: on this side both
        /// are simply a card with no code. Asynchronous because the real one
        /// signs and blocks on an HTTP round trip, which belongs on a queue of
        /// its own rather than on a thread of the cooperative pool.
        var voucher: (String) async -> String?
    }

    /// The held set moved: the banner has to be re-rendered.
    var didChange: (() -> Void)?

    private let logger = Logger(label: "WarrenLaunchAnnouncements")
    private let apiURL: URL?
    private let currentVersion: String
    private let backend: Backend
    private let lock = NSLock()
    private var held: WarrenHeldAnnouncements = .empty
    private var lastFetch: Date?
    private var timer: Timer?

    init(
        anchors: WarrenProductAnchors = .current,
        currentVersion: String = Bundle.main.productVersion,
        backend: Backend
    ) {
        self.apiURL = URL(string: anchors.apiURL.trimmingTrailingSlash() + "/v1/announcements")
        self.currentVersion = currentVersion
        self.backend = backend
    }

    /// The set to display at `now`: empty once the held envelope has lapsed.
    /// Pure, and it is the whole anti-freeze rule: a blocked or hostile
    /// network can suppress a card (it could suppress the whole API anyway)
    /// but can never freeze one on screen.
    static func displayable(_ held: WarrenHeldAnnouncements, now: Date) -> [WarrenAnnouncement] {
        now < held.activeUntil ? held.announcements : []
    }

    /// What the card must show right now.
    var announcements: [WarrenAnnouncement] {
        lock.withLock { Self.displayable(held, now: Date()) }
    }

    /// The app came to the foreground: fetch at once when due, then keep
    /// checking while it stays there.
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

    /// The app left the foreground. Nothing is fetched from the background.
    func stopPolling() {
        DispatchQueue.main.async { [weak self] in
            self?.timer?.invalidate()
            self?.timer = nil
        }
    }

    /// One fetch, when the interval has elapsed. The re-check on every
    /// activation goes through here too, so a resume a few seconds after the
    /// last poll does not make a second request.
    func refreshIfDue() async {
        let due = lock.withLock {
            warrenAnnouncementsFetchIsDue(
                lastFetch: lastFetch,
                now: Date(),
                interval: Self.checkInterval
            )
        }
        guard due else { return }
        await refresh()
    }

    /// One fetch, published to the held set.
    ///
    /// A refusal (no answer, or an envelope that did not verify) leaves the
    /// held set exactly as it is: taking a card down because the network or
    /// the boundary answered nonsense would let a transient failure erase an
    /// announcement the operator has not withdrawn. The set goes on its own
    /// when its `activeUntil` passes.
    func refresh() async {
        guard let apiURL else {
            logger.error("The API host could not be read; announcements stay unfetched")
            return
        }
        guard let body = await backend.fetch(apiURL) else {
            logger.debug("Announcements fetch failed")
            return
        }
        lock.withLock { lastFetch = Date() }
        guard let verified = backend.verify(body, currentVersion) else {
            // Rust has already logged the redacted reason. Never fall back to
            // the body: an unverified envelope is exactly the phishing case
            // the signature exists to stop.
            return
        }
        publish(
            WarrenHeldAnnouncements(
                announcements: await withCodes(verified.announcements),
                activeUntil: verified.activeUntil
            )
        )
    }

    /// The same announcements with this account's code attached to each one
    /// that offers a campaign. The wallet is touched only when an announcement
    /// actually carries an offer: the signed lookup is the one request here
    /// that is tied to an account, so it is never made speculatively.
    private func withCodes(_ announcements: [WarrenAnnouncement]) async -> [WarrenAnnouncement] {
        var withCodes: [WarrenAnnouncement] = []
        withCodes.reserveCapacity(announcements.count)
        for announcement in announcements {
            guard let campaign = announcement.voucherCampaignID else {
                withCodes.append(announcement)
                continue
            }
            var withCode = announcement
            withCode.voucherCode = await backend.voucher(campaign)
            withCodes.append(withCode)
        }
        return withCodes
    }

    private func publish(_ next: WarrenHeldAnnouncements) {
        let changed = lock.withLock { () -> Bool in
            let changed = held != next
            held = next
            return changed
        }
        guard changed else { return }
        didChange?()
    }
}

extension String {
    /// The base URL without its trailing slash, so a path can be appended to
    /// it without producing a double slash the server would not route.
    fileprivate func trimmingTrailingSlash() -> String {
        hasSuffix("/") ? String(dropLast()) : self
    }
}
