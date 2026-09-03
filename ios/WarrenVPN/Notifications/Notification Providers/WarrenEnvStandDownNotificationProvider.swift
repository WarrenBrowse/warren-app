//
//  WarrenEnvStandDownNotificationProvider.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import Foundation
import UIKit
import WarrenSettings
import WarrenTypes

/// The banner a build raises when a higher-priority product environment is
/// installed beside it: what it means, and the one way back.
///
/// Two states, because the presence of the other install is a URL scheme any
/// app can register and this build cannot authenticate it:
///
/// - the OFFER, before anything has been touched. Tapping it stands this build
///   down; the close button turns the offer down for good.
/// - the STAND-DOWN itself, once the user accepted it. Tapping it re-enables
///   this build for good (`WarrenEnvStandDown` keeps the other install marked
///   as seen), so the user is never trapped by an install they no longer use.
///
/// It outranks every other banner because while this build has stood down,
/// every tunnel message describes a tunnel it no longer holds, and only the
/// highest-priority descriptor is rendered.
final class WarrenEnvStandDownNotificationProvider: NotificationProvider,
    InAppNotificationProvider, @unchecked Sendable
{
    /// Pure display decision, so the trigger is exercised without UserDefaults.
    static func shouldDisplay(record: WarrenEnvStandDownRecord) -> Bool {
        record.isStandingDown || record.isOfferingStandDown
    }

    private let store: WarrenEnvStandDownStoring
    private let confirm: () -> Void
    private let reEnable: () -> Void

    init(
        store: WarrenEnvStandDownStoring,
        confirm: @escaping () -> Void,
        reEnable: @escaping () -> Void
    ) {
        self.store = store
        self.confirm = confirm
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
        let record = store.warrenEnvStandDown
        guard Self.shouldDisplay(record: record) else {
            return nil
        }
        return record.isStandingDown ? standingDownDescriptor() : offerDescriptor()
    }

    /// The offer. It names what accepting costs, and it is dismissible: the
    /// signal behind it cannot be authenticated, so a user who knows the other
    /// install is not Warren's must be able to put the banner away without
    /// giving anything up.
    private func offerDescriptor() -> InAppNotificationDescriptor {
        let body = NSLocalizedString(
            "Warren VPN is installed on this device and takes priority. Tap here to "
                + "stand this build down: it disconnects, stops connecting on demand "
                + "and turns “Force all apps” off.",
            comment: ""
        )

        return InAppNotificationDescriptor(
            identifier: identifier,
            style: .warning,
            title: NSLocalizedString("PRODUCTION HAS PRIORITY", comment: ""),
            body: NSAttributedString(string: body),
            button: InAppNotificationAction(
                image: UIImage.Buttons.closeSmall,
                handler: { [weak self] in
                    guard let self else { return }
                    reEnable()
                    invalidate()
                }
            ),
            tapAction: InAppNotificationAction(
                handler: { [weak self] in
                    guard let self else { return }
                    confirm()
                    invalidate()
                }
            )
        )
    }

    /// The stand-down in force: Connect is refused while this stands, so the
    /// banner has to carry the way back.
    private func standingDownDescriptor() -> InAppNotificationDescriptor {
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
