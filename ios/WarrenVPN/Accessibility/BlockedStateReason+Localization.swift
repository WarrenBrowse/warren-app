//
//  BlockedStateReason+Localization.swift
//  MullvadVPN
//
//  Created on 2026-02-16.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import Foundation
import PacketTunnelCore

extension BlockedStateReason {
    var localizedReason: String {
        switch self {
        case .outdatedSchema:
            NSLocalizedString(
                "Unable to start tunnel connection after update. Please disconnect and reconnect.",
                comment: ""
            )
        case .noRelaysSatisfyingFilterConstraints:
            NSLocalizedString("No servers match your location filter. Try changing filter settings.", comment: "")
        case .multihopEntryEqualsExit:
            NSLocalizedString(
                "The entry and exit servers cannot be the same. Try changing one to a new server or location.",
                comment: ""
            )
        case .noRelaysSatisfyingDaitaConstraints:
            NSLocalizedString(
                "No DAITA compatible servers match your location settings. Try changing location.",
                comment: ""
            )
        case .noRelaysSatisfyingObfuscationSettings:
            NSLocalizedString(
                "No servers match your obfuscation settings. Try changing location or obfuscation method.",
                comment: ""
            )
        case .noRelaysSatisfyingConstraints:
            NSLocalizedString("No servers match your settings, try changing server or other settings.", comment: "")
        case .noRelaysSatisfyingPortConstraints:
            NSLocalizedString(
                "The selected QUIC port is not supported, please change it under **VPN settings**.",
                comment: ""
            )
        case .noRelaysSatisfyingObfuscationPortConstraints:
            NSLocalizedString(
                "The selected obfuscation port is not supported, please change it under **VPN settings**.",
                comment: ""
            )
        case .invalidAccount:
            NSLocalizedString(
                "You are logged in with an invalid public key. Please log out and try another one.",
                comment: ""
            )
        case .deviceLoggedOut:
            NSLocalizedString("Unable to authenticate account. Please log out and log back in.", comment: "")
        case .accountBannedPortForwarding:
            Self.banMessage(portForwarding: true)
        case .accountBanned:
            Self.banMessage(portForwarding: false)
        default:
            NSLocalizedString(
                "Unable to start tunnel connection. Please report the problem on our community forum.",
                comment: ""
            )
        }
    }

    /// A suspension (warren-core doc 105), with the day it ends when the
    /// extension learned it, and for a port-forwarding ban the page that says
    /// how to contest it: a suspension, unlike an expiry, is not lifted by
    /// renewing.
    private static func banMessage(portForwarding: Bool) -> String {
        let lapsesAt = UserDefaults(suiteName: ApplicationConfiguration.securityGroupIdentifier)?
            .object(forKey: WarrenAppGroupKey.accountBanLapsesAt.rawValue) as? Date
        let until = lapsesAt.map { WarrenAccountStandingText.day($0) }
        let reportsURL = WarrenAccountStandingText.reportsURL
        switch (portForwarding, until) {
        case let (true, until?):
            return String(
                format: String(
                    localized: "Your access has been suspended until %1$@ after repeated abuse reports about a forwarded port. You can contest this at %2$@",
                    table: "Settings"
                ),
                until, reportsURL
            )
        case (true, nil):
            return String(
                format: String(
                    localized: "Your access has been suspended after repeated abuse reports about a forwarded port. You can contest this at %@",
                    table: "Settings"
                ),
                reportsURL
            )
        case let (false, until?):
            return String(
                format: String(
                    localized: "Your access has been suspended until %@ for a usage policy violation. Contact support if you believe this is a mistake.",
                    table: "Settings"
                ),
                until
            )
        case (false, nil):
            return String(
                localized: "Your access has been suspended for a usage policy violation. Contact support if you believe this is a mistake.",
                table: "Settings"
            )
        }
    }
}
