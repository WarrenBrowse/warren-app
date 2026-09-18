//
//  WarrenFailoverNotificationProvider.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import Foundation
import WarrenSettings
import WarrenTypes
import UIKit

/// Persisted high-water mark of the multi-exit failover count the user has
/// already acknowledged (dismissed). Kept behind a protocol so the banner
/// trigger logic can be unit tested against a fake store.
protocol WarrenFailoverAcknowledging: AnyObject {
    var warrenAcknowledgedFailoverCount: Int { get set }
}

extension AppPreferences: WarrenFailoverAcknowledging {}

/// Multi-exit failover banner. Shown whenever the daemon has reported more
/// failovers (an alternative exit was picked after the previous one became
/// unreachable) than the user has acknowledged. The live counter is written
/// by the PacketTunnel extension into App Group `UserDefaults`
/// (`WarrenAppGroupKey.failoverCount`); dismissing the banner persists the
/// current count so it stays dismissed until the next failover.
///
/// Warren differentiator (per `warren_competitor_comparatives`): Mullvad and
/// IVPN require the user to disconnect manually, Warren reroutes silently and
/// only surfaces this informational banner.
final class WarrenFailoverNotificationProvider: NotificationProvider,
    InAppNotificationProvider, @unchecked Sendable
{
    /// Pure display decision, mirroring the desktop provider's `mayDisplay`.
    /// Extracted so it can be exercised without UserDefaults or UIKit.
    static func shouldDisplay(failoverCount: Int, acknowledgedCount: Int) -> Bool {
        failoverCount > acknowledgedCount
    }

    private let acknowledgeStore: WarrenFailoverAcknowledging
    private let failoverCountReader: () -> Int
    private var foregroundObserver: NSObjectProtocol?
    private var backgroundObserver: NSObjectProtocol?
    private var pollTimer: DispatchSourceTimer?

    init(
        acknowledgeStore: WarrenFailoverAcknowledging,
        failoverCountReader: @escaping () -> Int = WarrenFailoverNotificationProvider.readFailoverCountFromAppGroup
    ) {
        self.acknowledgeStore = acknowledgeStore
        self.failoverCountReader = failoverCountReader
        super.init()
        observeForeground()
    }

    deinit {
        pollTimer?.cancel()
        for observer in [foregroundObserver, backgroundObserver].compactMap({ $0 }) {
            NotificationCenter.default.removeObserver(observer)
        }
    }

    override var identifier: NotificationProviderIdentifier {
        .warrenFailoverInAppNotification
    }

    override var priority: NotificationPriority {
        .high
    }

    var notificationDescriptor: InAppNotificationDescriptor? {
        let failoverCount = failoverCountReader()
        guard Self.shouldDisplay(
            failoverCount: failoverCount,
            acknowledgedCount: acknowledgeStore.warrenAcknowledgedFailoverCount
        ) else {
            return nil
        }

        return InAppNotificationDescriptor(
            identifier: identifier,
            style: .warning,
            title: NSLocalizedString("EXIT SWITCHED", comment: ""),
            body: NSAttributedString(
                string: NSLocalizedString(
                    "Your previous exit became unreachable. Warren routed you "
                        + "through an alternative server automatically.",
                    comment: ""
                )
            ),
            button: InAppNotificationAction(
                image: UIImage.Buttons.closeSmall,
                handler: { [weak self] in
                    guard let self else { return }
                    // Acknowledge the count observed at dismissal time so the
                    // banner reappears only on a subsequent failover.
                    acknowledgeStore.warrenAcknowledgedFailoverCount = failoverCountReader()
                    invalidate()
                }
            )
        )
    }

    /// The counter is written by the packet tunnel extension, a process of its
    /// own, and `UserDefaults.didChangeNotification` is posted only for writes
    /// made inside the receiving process: subscribing to it left this banner
    /// with no trigger at all. So read the counter back instead, and only while
    /// the app is in front, where the banner can actually be seen.
    private func observeForeground() {
        foregroundObserver = NotificationCenter.default.addObserver(
            forName: UIApplication.didBecomeActiveNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            self?.invalidate()
            self?.startPolling()
        }
        backgroundObserver = NotificationCenter.default.addObserver(
            forName: UIApplication.didEnterBackgroundNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            self?.stopPolling()
        }
    }

    private func startPolling() {
        stopPolling()
        let timer = DispatchSource.makeTimerSource(queue: .main)
        // The extension rewrites the suite every 2 seconds; matching it keeps
        // the banner within one write of the failover.
        timer.schedule(deadline: .now() + 2, repeating: 2)
        timer.setEventHandler { [weak self] in
            self?.invalidate()
        }
        timer.resume()
        pollTimer = timer
    }

    private func stopPolling() {
        pollTimer?.cancel()
        pollTimer = nil
    }

    private static func appGroupDefaults() -> UserDefaults? {
        let suite = Bundle.main.object(forInfoDictionaryKey: "ApplicationSecurityGroupIdentifier") as? String
        return suite.flatMap { UserDefaults(suiteName: $0) }
    }

    static func readFailoverCountFromAppGroup() -> Int {
        appGroupDefaults()?.integer(forKey: WarrenAppGroupKey.failoverCount.rawValue) ?? 0
    }
}
