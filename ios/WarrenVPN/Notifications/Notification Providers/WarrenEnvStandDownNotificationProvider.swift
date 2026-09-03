//
//  WarrenEnvStandDownNotificationProvider.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import Foundation
import WarrenSettings
import WarrenTypes

/// The banner a build raises once it has stood down for a higher-priority
/// product environment installed beside it: what happened, and the one way
/// back. Tapping it re-enables this build for good (`WarrenEnvStandDown`
/// keeps the other install marked as seen), so the user is never trapped by
/// an install they no longer use.
///
/// It outranks every other banner because while this build has stood down,
/// every tunnel message describes a tunnel it no longer holds, and only the
/// highest-priority descriptor is rendered.
final class WarrenEnvStandDownNotificationProvider: NotificationProvider,
    InAppNotificationProvider, @unchecked Sendable
{
    /// Pure display decision, so the trigger is exercised without UserDefaults.
    static func shouldDisplay(record: WarrenEnvStandDownRecord) -> Bool {
        record.isStandingDown
    }

    private let store: WarrenEnvStandDownStoring
    private let reEnable: () -> Void

    init(store: WarrenEnvStandDownStoring, reEnable: @escaping () -> Void) {
        self.store = store
        self.reEnable = reEnable
        super.init()
    }

    override var identifier: NotificationProviderIdentifier {
        .warrenEnvStandDownInAppNotification
    }

    override var priority: NotificationPriority {
        .exclusive
    }

    var notificationDescriptor: InAppNotificationDescriptor? {
        guard Self.shouldDisplay(record: store.warrenEnvStandDown) else {
            return nil
        }

        let body = [
            NSLocalizedString(
                "Warren VPN is installed on this device and takes priority, so this build "
                    + "disconnected, stopped connecting on demand and turned “Force all apps” off.",
                comment: ""
            ),
            NSLocalizedString("Tap here to use this build anyway.", comment: ""),
        ].joinedParagraphs()

        return InAppNotificationDescriptor(
            identifier: identifier,
            style: .warning,
            title: NSLocalizedString("PRODUCTION HAS PRIORITY", comment: ""),
            body: NSAttributedString(string: body),
            tapAction: InAppNotificationAction(
                handler: { [weak self] in
                    guard let self else { return }
                    reEnable()
                    invalidate()
                }
            )
        )
    }
}
