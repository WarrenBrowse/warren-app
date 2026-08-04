//
//  WarrenAppVersionGate.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  Forced-update gate driven by the signed `ios.json` update manifest,
//  the same Ed25519-signed metadata the desktop daemon and Android use.
//  The manifest is fetched at most once per day with URLSession, verified
//  in Rust (`WarrenVersionManifestVerifier`), and the verified verdict is
//  cached so the gate decision is available synchronously at launch.
//
//  Failure policy: no manifest (network error, never fetched) means no
//  gate, so a flaky network can never lock users out. A manifest that
//  fails signature verification is treated as absent too, but logged:
//  its content is never trusted in either direction.
//

import Foundation
import WarrenLogging
import WarrenRustRuntime
import WarrenTypes

/// App Store listing opened by the forced-update gate and the
/// "update available" flows.
///
/// PLACEHOLDER: this is the Mullvad VPN listing id (1488466513) inherited
/// from upstream, kept as the single constant to swap once Warren has its
/// own App Store listing.
enum WarrenAppStoreListing {
    static let url = URL(string: "itms-apps://itunes.apple.com/app/id1488466513")!
}

/// Last verified manifest verdict, persisted across launches.
struct WarrenVersionGateSnapshot: Codable, Equatable {
    /// Verdict of the shared minimum-version rule for `checkedAppVersion`.
    let supported: Bool
    /// Highest version listed in the manifest, if any.
    let latestVersion: String?
    /// App version the manifest was evaluated against. A snapshot taken
    /// for another version (the app got updated since) is stale.
    let checkedAppVersion: String
}

/// Whether a manifest fetch is due: never checked, or the last check is
/// older than `interval`. A future `lastCheck` (clock rollback) counts as
/// due rather than silencing checks for an arbitrary while.
func warrenVersionCheckIsDue(lastCheck: Date?, now: Date, interval: TimeInterval) -> Bool {
    guard let lastCheck else { return true }
    let elapsed = now.timeIntervalSince(lastCheck)
    return elapsed < 0 || elapsed >= interval
}

/// Whether the forced-update gate must engage. Fail-open: no snapshot, or
/// a snapshot taken for a different app version, never blocks.
func warrenUpdateGateShouldBlock(
    snapshot: WarrenVersionGateSnapshot?,
    currentAppVersion: String
) -> Bool {
    guard let snapshot, snapshot.checkedAppVersion == currentAppVersion else { return false }
    return !snapshot.supported
}

/// Latest manifest version strictly newer than the running one, for the
/// soft "update available" notice. `nil` when up to date or unknown.
func warrenUpdateAvailableVersion(
    snapshot: WarrenVersionGateSnapshot?,
    currentAppVersion: String
) -> String? {
    guard let latest = snapshot?.latestVersion else { return nil }
    let isNewer = latest.compare(currentAppVersion, options: .numeric) == .orderedDescending
    return isNewer ? latest : nil
}

/// Fetches, verifies and caches the signed iOS update manifest, and
/// answers the gate + update-available questions from the cached verdict.
final class WarrenAppVersionGate: @unchecked Sendable {
    static let shared = WarrenAppVersionGate()

    /// Same base as the other platforms' manifests
    /// (`mullvad-update::defaults::WARREN_RELEASES_URL`).
    private static let manifestURL = URL(string: "https://api.warrenbrowse.com/updates/desktop/ios.json")!
    private static let checkInterval: TimeInterval = Duration.days(1).timeInterval
    private static let snapshotKey = "WarrenVersionGateSnapshot"
    private static let lastCheckKey = "WarrenVersionGateLastCheck"

    private let logger = Logger(label: "WarrenAppVersionGate")
    private let defaults: UserDefaults
    private let currentAppVersion: String

    init(defaults: UserDefaults = .standard, currentAppVersion: String = Bundle.main.productVersion) {
        self.defaults = defaults
        self.currentAppVersion = currentAppVersion
    }

    /// Synchronous gate decision from the cached verified verdict.
    var isBlocked: Bool {
        warrenUpdateGateShouldBlock(snapshot: loadSnapshot(), currentAppVersion: currentAppVersion)
    }

    /// Newer manifest version for the soft "update available" notice.
    var updateAvailableVersion: String? {
        warrenUpdateAvailableVersion(snapshot: loadSnapshot(), currentAppVersion: currentAppVersion)
    }

    /// Fetches and verifies the manifest when the daily check is due, then
    /// returns the (possibly refreshed) gate decision.
    func refreshIfDue() async -> Bool {
        let due = warrenVersionCheckIsDue(
            lastCheck: defaults.object(forKey: Self.lastCheckKey) as? Date,
            now: Date(),
            interval: Self.checkInterval
        )
        guard due else { return isBlocked }

        do {
            var request = URLRequest(url: Self.manifestURL)
            request.timeoutInterval = 15
            let (data, _) = try await URLSession.shared.data(for: request)

            if let verified = WarrenVersionManifestVerifier.verify(
                manifest: data,
                currentVersion: currentAppVersion
            ) {
                storeSnapshot(
                    WarrenVersionGateSnapshot(
                        supported: verified.supported,
                        latestVersion: verified.latestVersion,
                        checkedAppVersion: currentAppVersion
                    )
                )
                defaults.set(Date(), forKey: Self.lastCheckKey)
            } else {
                // The content is untrusted, so it neither blocks nor clears
                // a previous block, but a signature failure on a healthy
                // fetch is suspicious enough to log. Marking the check done
                // avoids hammering a broken (or hostile) endpoint.
                logger.error("Update manifest failed signature verification; ignoring it")
                defaults.set(Date(), forKey: Self.lastCheckKey)
            }
        } catch {
            // Fail open, and leave the last-check timestamp untouched so the
            // next launch retries instead of waiting out the full interval.
            logger.debug("Update manifest fetch failed: \(error.localizedDescription)")
        }

        return isBlocked
    }

    private func loadSnapshot() -> WarrenVersionGateSnapshot? {
        guard let data = defaults.data(forKey: Self.snapshotKey) else { return nil }
        return try? JSONDecoder().decode(WarrenVersionGateSnapshot.self, from: data)
    }

    private func storeSnapshot(_ snapshot: WarrenVersionGateSnapshot) {
        guard let data = try? JSONEncoder().encode(snapshot) else { return }
        defaults.set(data, forKey: Self.snapshotKey)
    }
}
