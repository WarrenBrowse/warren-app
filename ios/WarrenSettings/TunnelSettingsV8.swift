//
//  TunnelSettingsV8.swift
//  MullvadVPN
//
//  Created by Andrew Bulhak on 2026-03-12.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import Foundation
import WarrenTypes

/// Whether NAT-PMP port forwarding through the Warren exit is enabled.
public enum WarrenNatPmpState: Codable, Sendable {
    case on
    case off

    public var isEnabled: Bool {
        get { self == .on }
        set { self = newValue ? .on : .off }
    }
}

/// NAT-PMP port-forwarding settings (Warren's differentiator vs the
/// Mullvad/IVPN abandonment of port forwarding). Default OFF: opening a
/// public port is an explicit opt-in, never a surprise. The protocol
/// (UDP) and external port (exit-picked, sticky across exit changes) are
/// fixed by the tunnel FFI for now, so the only knob is the toggle.
/// Declared next to `TunnelSettingsV8` (its first carrier) rather than in
/// its own file to keep the fork's Xcode project surface minimal.
public struct WarrenNatPmpSettings: Codable, Equatable, Sendable, CustomDebugStringConvertible {
    public var state: WarrenNatPmpState

    public var isEnabled: Bool {
        state.isEnabled
    }

    public init(state: WarrenNatPmpState = .off) {
        self.state = state
    }

    public var debugDescription: String {
        "WarrenNatPmpSettings(state: \(state))"
    }
}

/// Whether DNS queries to resolvers other than the tunnel-configured one
/// are allowed while connected.
public enum WarrenAllowExternalDnsState: Codable, Sendable {
    case on
    case off

    public var isEnabled: Bool {
        get { self == .on }
        set { self = newValue ? .on : .off }
    }
}

/// "Allow external DNS resolvers" (iOS port of the desktop VPN-settings
/// toggle). Default OFF: monopolizing DNS is the leak-safe posture, so
/// relaxing it is an explicit opt-in for advanced users testing remote
/// resolution. On iOS the knob controls whether the packet tunnel claims
/// every DNS query via `NEDNSSettings.matchDomains = [""]` (see
/// `PacketTunnelProvider.tunnelNetworkSettings`). Declared next to
/// `TunnelSettingsV8` (its first carrier) rather than in its own file to
/// keep the fork's Xcode project surface minimal.
public struct WarrenAllowExternalDnsSettings: Codable, Equatable, Sendable, CustomDebugStringConvertible {
    public var state: WarrenAllowExternalDnsState

    public var isEnabled: Bool {
        state.isEnabled
    }

    public init(state: WarrenAllowExternalDnsState = .off) {
        self.state = state
    }

    public var debugDescription: String {
        "WarrenAllowExternalDnsSettings(state: \(state))"
    }
}

public struct TunnelSettingsV8: Codable, Equatable, TunnelSettings, Sendable {
    /// Relay constraints.
    public var relayConstraints: RelayConstraints

    /// DNS settings.
    public var dnsSettings: DNSSettings

    /// WireGuard obfuscation settings
    public var wireGuardObfuscation: WireGuardObfuscationSettings

    /// Whether Post Quantum exchanges are enabled.
    public var tunnelQuantumResistance: TunnelQuantumResistance

    /// Whether Multihop is enabled.
    public var tunnelMultihopState: MultihopStateV2

    /// DAITA settings.
    public var daita: DAITASettings

    /// IAN settings.
    public var includeAllNetworks: IncludeAllNetworksSettings

    /// IP version preference for relay connections.
    public var ipVersion: IPVersion

    /// NAT-PMP port forwarding through the Warren exit.
    public var natPmp: WarrenNatPmpSettings

    /// Allow DNS queries to resolvers other than the tunnel-configured one.
    public var allowExternalDns: WarrenAllowExternalDnsSettings

    public var automaticMultihopIsEnabled: Bool {
        (tunnelMultihopState == .whenNeeded)
            || (tunnelMultihopState == .always && relayConstraints.entryLocations == .any)
    }

    public init(
        relayConstraints: RelayConstraints = RelayConstraints(),
        dnsSettings: DNSSettings = DNSSettings(),
        wireGuardObfuscation: WireGuardObfuscationSettings = WireGuardObfuscationSettings(),
        tunnelQuantumResistance: TunnelQuantumResistance = .on,
        tunnelMultihopState: MultihopStateV2 = .never,
        daita: DAITASettings = DAITASettings(),
        includeAllNetworks: IncludeAllNetworksSettings = IncludeAllNetworksSettings(),
        ipVersion: IPVersion = .automatic,
        natPmp: WarrenNatPmpSettings = WarrenNatPmpSettings(),
        allowExternalDns: WarrenAllowExternalDnsSettings = WarrenAllowExternalDnsSettings()
    ) {
        self.relayConstraints = relayConstraints
        self.dnsSettings = dnsSettings
        self.wireGuardObfuscation = wireGuardObfuscation
        self.tunnelQuantumResistance = tunnelQuantumResistance
        self.tunnelMultihopState = tunnelMultihopState
        self.daita = daita
        self.includeAllNetworks = includeAllNetworks
        self.ipVersion = ipVersion
        self.natPmp = natPmp
        self.allowExternalDns = allowExternalDns
    }

    public init(from decoder: any Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)

        self.relayConstraints =
            try container.decode(RelayConstraints.self, forKey: .relayConstraints)
        self.dnsSettings =
            try container.decode(DNSSettings.self, forKey: .dnsSettings)
        self.wireGuardObfuscation =
            try container.decode(WireGuardObfuscationSettings.self, forKey: .wireGuardObfuscation)
        self.tunnelQuantumResistance =
            try container.decode(TunnelQuantumResistance.self, forKey: .tunnelQuantumResistance)
        self.tunnelMultihopState =
            try container.decode(MultihopStateV2.self, forKey: .tunnelMultihopState)
        self.daita =
            try container.decode(DAITASettings.self, forKey: .daita)
        self.includeAllNetworks =
            (try? container.decode(IncludeAllNetworksSettings.self, forKey: .includeAllNetworks))
            ?? IncludeAllNetworksSettings()
        self.ipVersion =
            (try? container.decode(IPVersion.self, forKey: .ipVersion))
            ?? .automatic
        // Lenient decode (like includeAllNetworks / ipVersion above): a
        // stored V8 payload written before this field existed decodes to
        // the safe default (port forwarding off) instead of forcing a V9
        // schema bump for a purely additive field.
        self.natPmp =
            (try? container.decode(WarrenNatPmpSettings.self, forKey: .natPmp))
            ?? WarrenNatPmpSettings()
        // Same lenient decode: a payload stored before this field existed
        // decodes to the safe default (DNS stays monopolized).
        self.allowExternalDns =
            (try? container.decode(WarrenAllowExternalDnsSettings.self, forKey: .allowExternalDns))
            ?? WarrenAllowExternalDnsSettings()
    }

    public func upgradeToNextVersion() -> any TunnelSettings {
        self
    }

    public var debugDescription: String {
        "TunnelSettingsV8(relayConstraints: \(self.relayConstraints), dnsSettings: \(self.dnsSettings), wireGuardObfuscation: \(self.wireGuardObfuscation), tunnelQuantumResistance: \(self.tunnelQuantumResistance), tunnelMultihopState: \(self.tunnelMultihopState), daita: \(self.daita), includeAllNetworks: \(self.includeAllNetworks.debugDescription))"
    }
}
