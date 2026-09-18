//
//  ChipFeature.swift
//  MullvadVPN
//
//  Created by Mojgan on 2024-12-06.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//
import WarrenRustRuntime
import WarrenSettings
import WarrenTypes
import PacketTunnelCore
import SwiftUI

protocol ChipFeature: Identifiable {
    var id: FeatureType { get }
    var isEnabled: Bool { get }
    var name: String { get }
    var icon: Image? { get }
}

extension ChipFeature {
    var icon: Image? { nil }
}

enum FeatureType {
    case daita
    case multihop
    case quantumResistance
    case obfuscation
    case dns
    case ipOverrides
    case includeAllNetworks
    case localNetworkSharing
    case ipVersion
    case portForwarding
    case allowExternalDns
}

/// The DAITA chip, which on this client can only ever say the defense is NOT
/// running.
///
/// `TunnelState.isDaita` carries the settings toggle, not a grant: the iOS
/// datapath never negotiates DAITA at all
/// (`warren_tunnel_ffi.rs` dials with the defense off, and the comment there
/// says so), so a plain "DAITA" chip claimed a protection that was not on the
/// wire. In a privacy product that is the worst kind of wrong. The desktop
/// daemon reads the exit's own echo (`primary().daita_spec()`) and refuses to
/// claim what is not running; until iOS carries that echo too, this chip says
/// what is true, in the same words the other two clients use
/// (`features.rs`, `feature_daita_not_active_on_server`).
///
/// `DaitaTruthfulnessTests` holds this to the datapath: when iOS starts
/// negotiating DAITA, that test is what tells the next reader to revisit here.
struct DaitaFeature: ChipFeature {
    let id: FeatureType = .daita
    let state: TunnelState
    let settings: LatestTunnelSettings

    /// Whether the live session actually carries a granted DAITA machine.
    ///
    /// Read from the datapath rather than assumed: the setting says what was
    /// asked for, and an exit answers per session. A chip drawn from the
    /// setting claimed a defense that was not running, which is the one thing
    /// a privacy indicator must never do.
    var isGranted: Bool = WarrenQuinnAdapter.daitaActive()

    /// Shown whenever the user asked for DAITA, because the point of the chip
    /// is to tell them whether it is happening.
    var isEnabled: Bool {
        settings.daita.isEnabled
    }

    /// What the chip says: the grant when the session carries one, and the
    /// plain statement that it is not running otherwise.
    var name: String {
        isGranted
            ? NSLocalizedString("DAITA", comment: "")
            : NSLocalizedString("DAITA: not active on this server", comment: "")
    }
}

struct QuantumResistanceFeature: ChipFeature {
    let id: FeatureType = .quantumResistance
    let state: TunnelState

    var isEnabled: Bool {
        state.isPostQuantum ?? false
    }

    var name: String {
        NSLocalizedString("Quantum resistance", comment: "")
    }
}

struct MultihopFeature: ChipFeature {
    let id: FeatureType = .multihop
    let state: TunnelState
    let settings: LatestTunnelSettings

    var isEnabled: Bool {
        state.isMultihop
    }

    var name: String {
        NSLocalizedString("Multihop", comment: "")
    }

    var icon: Image? {
        settings.tunnelMultihopState.isWhenNeeded ? .warrenIconMultihopWhenNeeded : nil
    }
}

struct ObfuscationFeature: ChipFeature {
    let id: FeatureType = .obfuscation
    let settings: LatestTunnelSettings
    let state: ObservedState

    var actualObfuscationMethod: ObfuscationMethod {
        state.connectionState.map { $0.obfuscationMethod } ?? .off
    }

    var isEnabled: Bool {
        actualObfuscationMethod.isEnabled
    }

    var isAutomatic: Bool {
        settings.wireGuardObfuscation.state == .automatic
    }

    var name: String {
        // This just currently says "Obfuscation".
        // To add an automaticity indicator (a trailing " (automatic)"
        // or a colour/border style or whatever), use the `isAutomatic` field.
        // To say what type of obfuscation it is,
        // we can look at `actualObfuscationMethod`
        NSLocalizedString("Obfuscation", comment: "")
    }
}

struct DNSFeature: ChipFeature {
    let id: FeatureType = .dns
    let settings: LatestTunnelSettings

    var isEnabled: Bool {
        settings.dnsSettings.enableCustomDNS || !settings.dnsSettings.blockingOptions.isEmpty
    }

    var name: String {
        if !settings.dnsSettings.blockingOptions.isEmpty {
            NSLocalizedString("DNS content blockers", comment: "")
        } else {
            NSLocalizedString("Custom DNS", comment: "")
        }
    }
}

struct IPOverrideFeature: ChipFeature {
    let id: FeatureType = .ipOverrides
    let state: TunnelState

    var isEnabled: Bool {
        guard let selectedRelays = state.relays else {
            return false
        }
        return (selectedRelays.entry?.isIPOverridden ?? false) || selectedRelays.exit.isIPOverridden
    }

    var name: String {
        NSLocalizedString("Server IP override", comment: "")
    }
}

struct IncludeAllNetworksFeature: ChipFeature {
    let id: FeatureType = .includeAllNetworks
    let settings: LatestTunnelSettings

    var isEnabled: Bool {
        let settings = IncludeAllNetworksSettings(
            includeAllNetworksState: settings.includeAllNetworks.includeAllNetworksState,
            localNetworkSharingState: settings.includeAllNetworks.localNetworkSharingState
        )

        return settings.includeAllNetworksIsEnabled
    }

    var name: String {
        NSLocalizedString("Force all apps", comment: "")
    }
}

struct LocalNetworkSharingFeature: ChipFeature {
    let id: FeatureType = .localNetworkSharing
    let settings: LatestTunnelSettings

    var isEnabled: Bool {
        let settings = IncludeAllNetworksSettings(
            includeAllNetworksState: settings.includeAllNetworks.includeAllNetworksState,
            localNetworkSharingState: settings.includeAllNetworks.localNetworkSharingState
        )

        return settings.localNetworkSharingIsEnabled
    }

    var name: String {
        NSLocalizedString("Local network sharing", comment: "")
    }
}

struct PortForwardingFeature: ChipFeature {
    let id: FeatureType = .portForwarding
    let settings: LatestTunnelSettings

    var isEnabled: Bool {
        settings.natPmp.isEnabled
    }

    var name: String {
        NSLocalizedString("Port forwarding", comment: "")
    }
}

struct AllowExternalDnsFeature: ChipFeature {
    let id: FeatureType = .allowExternalDns
    let settings: LatestTunnelSettings

    var isEnabled: Bool {
        settings.allowExternalDns.isEnabled
    }

    var name: String {
        NSLocalizedString("Allow external DNS", comment: "")
    }
}

struct IPVersionFeature: ChipFeature {
    let id: FeatureType = .ipVersion
    let state: TunnelState

    var isEnabled: Bool {
        // Show IPv6 indicator when the ingress endpoint is using IPv6
        guard let endpoint = state.relays?.ingress.endpoint else { return false }
        if case .ipv6 = endpoint.socketAddress {
            return true
        }
        return false
    }

    var name: String {
        NSLocalizedString("IPv6", comment: "")
    }
}
