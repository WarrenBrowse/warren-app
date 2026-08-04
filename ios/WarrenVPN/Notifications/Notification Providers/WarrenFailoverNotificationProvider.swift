//
//  WarrenFailoverNotificationProvider.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import Foundation
import WarrenSettings
import WarrenTypes
import UIKit.UIImage

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
    private var observer: NSObjectProtocol?

    init(
        acknowledgeStore: WarrenFailoverAcknowledging,
        failoverCountReader: @escaping () -> Int = WarrenFailoverNotificationProvider.readFailoverCountFromAppGroup
    ) {
        self.acknowledgeStore = acknowledgeStore
        self.failoverCountReader = failoverCountReader
        super.init()
        addAppGroupObserver()
    }

    deinit {
        if let observer {
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

    private func addAppGroupObserver() {
        guard let defaults = Self.appGroupDefaults() else { return }
        observer = NotificationCenter.default.addObserver(
            forName: UserDefaults.didChangeNotification,
            object: defaults,
            queue: .main
        ) { [weak self] _ in
            self?.invalidate()
        }
    }

    private static func appGroupDefaults() -> UserDefaults? {
        let suite = Bundle.main.object(forInfoDictionaryKey: "ApplicationSecurityGroupIdentifier") as? String
        return suite.flatMap { UserDefaults(suiteName: $0) }
    }

    static func readFailoverCountFromAppGroup() -> Int {
        appGroupDefaults()?.integer(forKey: WarrenAppGroupKey.failoverCount.rawValue) ?? 0
    }
}
