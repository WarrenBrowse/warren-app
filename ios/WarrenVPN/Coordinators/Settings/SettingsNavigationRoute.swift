//
//  SettingsNavigationRoute.swift
//  MullvadVPN
//
//  Created by Jon Petersson on 2026-03-10.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

/// Settings navigation route.
enum SettingsNavigationRoute: Equatable {
    /// The route that's always displayed first upon entering settings.
    case root

    /// VPN settings.
    case vpnSettings

    /// FAQ section displayed as a modal safari browser.
    case faq

    /// API access route.
    case apiAccess

    /// changelog route.
    case changelog

    /// Multihop route.
    case multihop

    /// DAITA route.
    case daita

    /// Language route.
    case language

    /// Notification settings route.
    case notificationSettings

    /// IAN route.
    case includeAllNetworks

    /// Warren wallet backup (View recovery phrase, Face ID gated).
    case warrenWalletBackup

    /// Warren wallet wipe (destructive, requires confirmation).
    case warrenWalletErase

    /// Warren wallet identity (read-only pubkey display for support).
    case warrenWalletIdentity

    /// Warren tunnel statistics (status + bytes + duration + failover count).
    case warrenTunnelStatistics

    /// Warren diagnostic info (app version + wallet pubkey + tunnel stats
    /// in a screenshot-friendly support-ticket payload).
    case warrenDiagnosticInfo

    /// About Warren (marketing site + privacy + ToS + AGPL source links).
    case warrenAbout

    /// Forum sign-in finished with the code the approval page shows.
    case warrenForumSignInCode

    /// Warren NAT-PMP port forwarding settings.
    case warrenPortForwarding
}
