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

/// Which transport a forwarded port carries.
public enum WarrenNatPmpProtocol: String, Codable, Sendable, CaseIterable {
    case udp
    case tcp

    public var isTcp: Bool { self == .tcp }
}

/// NAT-PMP port-forwarding settings (Warren's differentiator vs the
/// Mullvad/IVPN abandonment of port forwarding). Default OFF: opening a
/// public port is an explicit opt-in, never a surprise.
/// Declared next to `TunnelSettingsV8` (its first carrier) rather than in
/// its own file to keep the fork's Xcode project surface minimal.
public struct WarrenNatPmpSettings: Codable, Equatable, Sendable, CustomDebugStringConvertible {
    public var state: WarrenNatPmpState

    /// The transport the mapping is for. The exit keys allocations by
    /// external port and refuses a different-protocol request on one it
    /// already holds, so this is part of what identifies the mapping.
    public var networkProtocol: WarrenNatPmpProtocol

    /// The port the user asked the exit for, or 0 to let it pick. A pin is
    /// honour-or-error at the exit, so a conflict is visible instead of
    /// silently landing on another port; 0 carries the last granted port
    /// over so the public port follows the client across an exit change.
    public var externalPort: UInt16

    /// Requested lease length. The client renews at half of what the exit
    /// actually granted, which may be less than this.
    public var lifetimeSeconds: UInt32

    /// The lease lengths the screen offers, and the one a record with none
    /// of them decodes to.
    public static let lifetimeChoices: [UInt32] = [3600, 21600, 86400]

    /// The port range an exit will consider. Below 1024 is the privileged
    /// range no exit hands out.
    public static let portRange: ClosedRange<UInt16> = 1024...65535

    public var isEnabled: Bool {
        state.isEnabled
    }

    public init(
        state: WarrenNatPmpState = .off,
        networkProtocol: WarrenNatPmpProtocol = .udp,
        externalPort: UInt16 = 0,
        lifetimeSeconds: UInt32 = 3600
    ) {
        self.state = state
        self.networkProtocol = networkProtocol
        self.externalPort = externalPort
        self.lifetimeSeconds = lifetimeSeconds
    }

    /// A record written before the protocol, port and lifetime were
    /// settable carries none of them. Each missing field decodes to the
    /// behaviour that record actually had, so an upgrade never changes a
    /// live mapping.
    public init(from decoder: any Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        state = try container.decode(WarrenNatPmpState.self, forKey: .state)
        networkProtocol =
            try container.decodeIfPresent(WarrenNatPmpProtocol.self, forKey: .networkProtocol) ?? .udp
        externalPort = try container.decodeIfPresent(UInt16.self, forKey: .externalPort) ?? 0
        lifetimeSeconds = try container.decodeIfPresent(UInt32.self, forKey: .lifetimeSeconds) ?? 3600
    }

    public var debugDescription: String {
        // The port is the user's own choice, not identity material, and it
        // is what a support log needs to explain a refused mapping.
        "WarrenNatPmpSettings(state: \(state), protocol: \(networkProtocol.rawValue), "
            + "port: \(externalPort), lifetime: \(lifetimeSeconds)s)"
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
