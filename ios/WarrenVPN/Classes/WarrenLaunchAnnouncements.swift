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

/// What the wallet-signed campaign lookup answered for this account. A card
/// with no code either way, and the difference is only whether asking again
/// could ever change the answer. The same three outcomes the daemon and
/// Android hold, under the same names.
enum WarrenCampaignVoucherAnswer: Equatable {
    /// The code drawn for this account.
    case drawn(String)
    /// The server answered: this account is outside the cohort, for good.
    case outside
    /// No answer came back, so the next poll asks again.
    case unanswered
}

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
        /// Warren SS58 address of the identity the voucher lookup would sign
        /// with, `nil` when this device holds no wallet at all. It keys the
        /// held codes and nothing else, and it never reaches a log.
        var address: () async -> String?

        /// This account's code for `campaignID`. Asynchronous because the real
        /// one signs and blocks on an HTTP round trip, which belongs on a queue
        /// of its own rather than on a thread of the cooperative pool.
        var voucher: (String) async -> WarrenCampaignVoucherAnswer
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

    /// The identity the held codes were drawn for, and what the signed lookup
    /// answered for each campaign under it.
    ///
    /// In memory only, and dropped whole on an identity change: a code belongs
    /// to the wallet that asked for it, and the wallet that replaces it has its
    /// own or none at all. Re-drawing is always safe because the server call is
    /// a pure lookup that never mints and never assigns.
    ///
    /// Deliberately not zeroized. The code is rendered on the reader's own
    /// screen and travels through the card to get there, so wiping this one
    /// copy would be theatre; what bounds the exposure is that it never reaches
    /// a log, an error or a problem report.
    private var codesAddress: String?
    private var codesByCampaign: [String: WarrenCampaignVoucherAnswer] = [:]

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
    ///
    /// The identity is read BEFORE anything else, and every code drawn under
    /// another one is dropped first. That ordering is the daemon's
    /// (`WarrenAnnouncementsUpdater::claim`) and Android's (`SessionCodes`):
    /// acting under a stale identity would show one account the offer drawn
    /// for another.
    private func withCodes(_ announcements: [WarrenAnnouncement]) async -> [WarrenAnnouncement] {
        guard announcements.contains(where: { $0.voucherCampaignID != nil }) else {
            return announcements
        }
        guard let address = await backend.address() else { return announcements }
        lock.withLock {
            if codesAddress != address {
                codesAddress = address
                codesByCampaign = [:]
            }
        }
        var withCodes: [WarrenAnnouncement] = []
        withCodes.reserveCapacity(announcements.count)
        for announcement in announcements {
            guard let campaign = announcement.voucherCampaignID else {
                withCodes.append(announcement)
                continue
            }
            var withCode = announcement
            withCode.voucherCode = await claim(campaign, drawnFor: address)
            withCodes.append(withCode)
        }
        return withCodes
    }

    /// This account's code for `campaign`, `nil` when there is none to show.
    ///
    /// The answer is held for the session, so the wallet-signed request goes
    /// out once rather than on every five minute refresh. Repeating it would
    /// turn a one-shot draw into a wallet-authenticated presence beacon: the
    /// backend would learn roughly when this wallet has the app open, twelve
    /// times an hour, for the life of the campaign. The announcements
    /// themselves ride a document byte-identical for every caller, so nothing
    /// else in this poll says anything about the account.
    ///
    /// Outside the cohort is held too, because it is a final server answer
    /// rather than a failure. A lookup that never answered is held by nothing,
    /// so a transient outage never tells a cohort member they were never
    /// eligible.
    private func claim(_ campaign: String, drawnFor address: String) async -> String? {
        if let answer = lock.withLock({ codesByCampaign[campaign] }) {
            return code(of: answer)
        }
        let answer = await backend.voucher(campaign)
        guard answer != .unanswered else { return nil }
        // The wallet can be replaced while the request is in flight (create,
        // restore, logout). A code drawn for the previous identity is not this
        // one's, so it is neither shown nor held.
        guard await backend.address() == address else { return nil }
        lock.withLock {
            guard codesAddress == address else { return }
            codesByCampaign[campaign] = answer
        }
        return code(of: answer)
    }

    private func code(of answer: WarrenCampaignVoucherAnswer) -> String? {
        guard case let .drawn(code) = answer else { return nil }
        return code
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
