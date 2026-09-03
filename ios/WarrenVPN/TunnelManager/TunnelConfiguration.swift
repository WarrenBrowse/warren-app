//
//  TunnelConfiguration.swift
//  MullvadVPN
//
//  Created by pronebird on 07/12/2022.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import Foundation
import NetworkExtension
import WarrenRustRuntime

struct TunnelConfiguration {
    var isEnabled: Bool
    var localizedDescription: String
    var protocolConfiguration: NETunnelProviderProtocol
    var onDemandRules: [NEOnDemandRule]
    var isOnDemandEnabled: Bool

    /// The name iOS shows for this VPN configuration in Settings, General, VPN
    /// and Device Management. It is the only status surface the app can brand:
    /// the status-bar VPN chip is drawn by the system with no app-supplied
    /// content, so with two installs on one device that row is the single
    /// place a user can tell which product holds the tunnel. Prod keeps the
    /// shipped string exactly: renaming its configuration would rewrite the
    /// row of every install that already has one.
    static func vpnConfigurationName(for anchors: WarrenProductAnchors = .current) -> String {
        let transport = "QUIC"
        guard let marker = anchors.environmentBadge else { return transport }
        return "\(transport) (\(marker.capitalized))"
    }

    init(includeAllNetworks: Bool, excludeLocalNetworks: Bool, isOnDemandEnabled: Bool = true) {
        let protocolConfig = NETunnelProviderProtocol()
        protocolConfig.providerBundleIdentifier = ApplicationTarget.packetTunnel.bundleIdentifier
        protocolConfig.serverAddress = ""
        protocolConfig.includeAllNetworks = includeAllNetworks
        protocolConfig.excludeLocalNetworks = excludeLocalNetworks

        let alwaysOnRule = NEOnDemandRuleConnect()
        alwaysOnRule.interfaceTypeMatch = .any

        isEnabled = true
        localizedDescription = Self.vpnConfigurationName()
        protocolConfiguration = protocolConfig
        onDemandRules = [alwaysOnRule]
        self.isOnDemandEnabled = isOnDemandEnabled
    }

    func apply(to manager: TunnelProviderManagerType) {
        manager.isEnabled = isEnabled
        manager.localizedDescription = localizedDescription
        manager.protocolConfiguration = protocolConfiguration
        manager.onDemandRules = onDemandRules
        manager.isOnDemandEnabled = isOnDemandEnabled
    }
}
