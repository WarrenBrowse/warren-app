//
//  AppRoutes.swift
//  MullvadVPN
//
//  Created by pronebird on 17/08/2023.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import Routing
import UIKit

/// Pure routing decision for the wallet identity model. "Logged in" means a
/// wallet exists in the Keychain; the legacy Mullvad account-number login is
/// gone. Order: the forced-update gate (a version below the verified
/// manifest minimum blocks everything else), then terms of service, then a
/// revoked device, then wallet presence (no wallet routes to the Create /
/// Restore login screen), then the one-time onboarding wizard, otherwise
/// the main UI. The wizard runs AFTER login (desktop parity): its wallet
/// step is a backup reminder of the existing phrase, and its subscription
/// step needs the wallet to mint a payment session.
func nextWarrenRoutes(
    updateBlocked: Bool,
    agreedToTOS: Bool,
    onboardingComplete: Bool,
    isRevoked: Bool,
    walletExists: Bool,
    isExpired: Bool
) -> [AppRoute] {
    guard !updateBlocked else { return [.blockedUpdate] }
    guard agreedToTOS else { return [.tos] }
    if isRevoked { return [.revoked] }
    guard walletExists else { return [.login] }
    guard onboardingComplete else { return [.warrenOnboarding] }
    // The wizard branch wins over this one: fresh wallets always start
    // unfunded and the wizard's subscription step is their funding flow.
    if isExpired { return [.main, .outOfTime] }
    return [.main]
}

/**
 Enum type describing groups of routes.
 */
enum AppRouteGroup: AppRouteGroupProtocol {
    /**
     Primary horizontal navigation group.
     */
    case primary

    /**
     Select location group.
     */
    case selectLocation

    /**
     Account group.
     */
    case account

    /**
     Settings group.
     */
    case settings

    /**
     Changelog group.
     */
    case changelog

    /**
     Alert group. Alert id should match the id of the alert being contained.
     */
    case alert(_ alertId: String)

    var isModal: Bool {
        switch self {
        case .primary:
            return false

        case .selectLocation, .account, .settings, .changelog, .alert:
            return true
        }
    }

    var modalLevel: Int {
        switch self {
        case .primary:
            return 0
        case .account, .selectLocation, .changelog:
            return 1
        case .settings:
            return 2
        case .alert:
            // Alerts should always be topmost.
            return .max
        }
    }
}

/**
 Enum type describing primary application routes.
 */
enum AppRoute: AppRouteProtocol {
    /**
     Account route.
     */
    case account

    /**
     Settings route. Contains sub-route to display.
     */
    case settings(SettingsNavigationRoute?)

    /**
     Settings route. Contains sub-route to display.
     */
    case vpnSettings(VPNSettingsSection?)

    /**
     Multihop standalone route (not subsetting).
     */
    case multihop

    /**
     DNS settings standalone route (not subsetting).
     */
    case dnsSettings

    /**
     Ip overrides standalone route (not subsetting).
     */
    case ipOverrides

    /**
     DAITA standalone route (not subsetting).
     */
    case daita

    /**
     IAN standalone route (not subsetting).
     */
    case includeAllNetworks

    /**
     Select location route.
     */
    case selectLocation

    /**
     Changelog standalone route (not subsetting).
     */
    case changelog

    /**
     API access methods standalone route (not subsetting).
     */
    case apiAccess

    /**
     Alert route. Alert id must be a unique string in order to produce a unique route
     that distinguishes between different kinds of alerts.
     */
    case alert(_ alertId: String)

    /**
     Routes that are part of primary horizontal navigation group.
     */
    case tos, login, main, revoked, outOfTime

    /**
     Warren onboarding wizard. Presented full-screen after TOS, before
     the regular login / welcome / main flow, so the wallet exists
     before any tunnel attempt.
     */
    case warrenOnboarding

    /**
     Forced-update gate. Presented when the verified update manifest
     reports the running version is below the minimum supported version.
     Terminal: never dismissed, the only exit is updating the app.
     */
    case blockedUpdate

    var isExclusive: Bool {
        switch self {
        case .account, .settings, .alert:
            return true
        default:
            return false
        }
    }

    var supportsSubNavigation: Bool {
        if case .settings = self {
            return true
        } else {
            return false
        }
    }

    var routeGroup: AppRouteGroup {
        switch self {
        case .tos, .login, .main, .revoked, .outOfTime, .warrenOnboarding, .blockedUpdate:
            return .primary
        case .selectLocation:
            return .selectLocation
        case .account:
            return .account
        case .settings, .daita, .changelog, .vpnSettings, .multihop, .dnsSettings, .ipOverrides,
            .includeAllNetworks, .apiAccess:
            return .settings
        case let .alert(id):
            return .alert(id)
        }
    }
}
