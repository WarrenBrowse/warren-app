//
//  ConnectionViewViewModel.swift
//  MullvadVPN
//
//  Created by Jon Petersson on 2024-12-09.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import Combine
import WarrenREST
import WarrenSettings
import WarrenTypes
import SwiftUI

/// The connect screen collapses the tunnel states into five visual phases, each
/// with its own accent color. Single source of truth (mirrors the desktop
/// connection-phase mapping) so the scenery backdrop, the status eye, the status
/// label and the header bar never drift apart.
///   exposed     : traffic in the clear (terracotta)
///   connecting  : tunnel coming up or down (orange)
///   protected   : tunnel up (olive)
///   interrupted : no network available, nothing can flow (orange)
///   blocked     : kill switch holds traffic, nothing leaks but nothing flows (neutral)
enum ConnectionPhase {
    case exposed
    case connecting
    case protected
    case interrupted
    case blocked

    init(tunnelState: TunnelState) {
        switch tunnelState {
        case .connected:
            self = .protected
        case .connecting, .reconnecting, .negotiatingEphemeralPeer, .pendingReconnect, .disconnecting:
            self = .connecting
        case .disconnected:
            self = .exposed
        case .waitingForConnectivity(.noNetwork):
            self = .interrupted
        case .waitingForConnectivity(.noConnection), .error:
            // Every iOS blocked-state reason is fail-closed (the packet tunnel
            // holds the default route), so an error is never "exposed".
            self = .blocked
        }
    }

    var accentColor: UIColor {
        switch self {
        case .protected: .successColor
        case .connecting, .interrupted: .pendingColor
        case .exposed: .dangerColor
        case .blocked: .white
        }
    }

    // A crossed-out eye reads as protected/hidden in the burrow (secured, blocked,
    // or the no-network hold where nothing can flow); an open eye reads as
    // exposed/visible. Mirrors the desktop status eye.
    var showsCrossedEye: Bool {
        switch self {
        case .protected, .blocked, .interrupted: true
        case .exposed, .connecting: false
        }
    }
}

class ConnectionViewViewModel: ObservableObject {
    enum TunnelActionButton {
        case connect
        case disconnect
        case cancel
    }

    enum TunnelAction {
        case connect
        case disconnect
        case cancel
        case reconnect
        case selectLocation
        case shuffleLocation
    }

    @Published private(set) var tunnelStatus: TunnelStatus
    @Published var showsActivityIndicator = false

    @Published var relayConstraints: RelayConstraints
    let destinationDescriber: DestinationDescribing

    var tunnelIsConnected: Bool {
        if case .connected = tunnelStatus.state {
            true
        } else {
            false
        }
    }

    var connectionName: String? {
        if case let .only(loc) = relayConstraints.exitLocations {
            return destinationDescriber.describe(loc)
        }
        return nil
    }

    init(
        tunnelStatus: TunnelStatus,
        relayConstraints: RelayConstraints,
        relayCache: RelayCacheProtocol,
        customListRepository: CustomListRepositoryProtocol
    ) {
        self.tunnelStatus = tunnelStatus
        self.relayConstraints = relayConstraints
        self.destinationDescriber = DestinationDescriber(
            relayCache: relayCache,
            customListRepository: customListRepository
        )
    }

    func update(tunnelStatus: TunnelStatus) {
        self.tunnelStatus = tunnelStatus
    }
}

extension ConnectionViewViewModel {
    var showsConnectionDetails: Bool {
        switch tunnelStatus.state {
        case .connecting, .reconnecting, .negotiatingEphemeralPeer,
            .connected, .pendingReconnect:
            true
        case .disconnecting, .disconnected, .waitingForConnectivity, .error:
            false
        }
    }

    var connectionPhase: ConnectionPhase {
        ConnectionPhase(tunnelState: tunnelStatus.state)
    }

    var textColorForSecureLabel: UIColor {
        connectionPhase.accentColor
    }

    var eyeSymbolName: String {
        connectionPhase.showsCrossedEye ? "eye.slash.fill" : "eye.fill"
    }

    var disableButtons: Bool {
        if case .waitingForConnectivity(.noNetwork) = tunnelStatus.state {
            true
        } else {
            false
        }
    }

    // Status copy mirrors the desktop connection card: a bold truth statement
    // ("Connection established" / "You are visible") plus a smaller factual
    // subtitle, instead of the upstream ALL-CAPS state names.
    var localizedTitleForSecureLabel: LocalizedStringKey {
        switch tunnelStatus.state {
        case .connected:
            LocalizedStringKey("Connection established")
        case .connecting, .reconnecting, .negotiatingEphemeralPeer, .pendingReconnect,
            .disconnecting, .disconnected:
            LocalizedStringKey("You are visible")
        case .waitingForConnectivity(.noConnection), .error:
            LocalizedStringKey("BLOCKED CONNECTION")
        case .waitingForConnectivity(.noNetwork):
            LocalizedStringKey("Connection interrupted")
        }
    }

    var localizedSubtitleForSecureLabel: LocalizedStringKey? {
        switch tunnelStatus.state {
        case .connected:
            LocalizedStringKey("You are protected")
        case .connecting, .reconnecting, .negotiatingEphemeralPeer, .pendingReconnect,
            .disconnecting(.reconnect):
            LocalizedStringKey("Connection in progress")
        case .disconnecting(.nothing):
            LocalizedStringKey("Disconnecting...")
        case .disconnected:
            LocalizedStringKey("Your connection is not encrypted")
        case .waitingForConnectivity(.noNetwork):
            // Still true during the hold: the kill switch keeps everything
            // fail-closed while waiting for the network to come back.
            LocalizedStringKey("You are protected")
        case .waitingForConnectivity(.noConnection), .error:
            nil
        }
    }

    // Emoji flag of the country the user currently appears in, hugging the
    // card's top-right corner in every state (desktop CurrentCountryFlag).
    // With a tunnel up or coming up that is the exit country; without one
    // there is no geoip source at all, so the OS locale region stands in for
    // the user's own country: offline, no network call, no leak.
    var currentCountryFlagEmoji: String? {
        let code = tunnelStatus.state.relays?.exit.location.countryCode
            ?? Locale.current.region?.identifier
        return code.flatMap(Self.flagEmoji(countryCode:))
    }

    private static func flagEmoji(countryCode: String) -> String? {
        let letterA: Unicode.Scalar = "A"
        let regionalIndicatorA: UInt32 = 0x1F1E6
        var flag = ""
        for letter in countryCode.uppercased().unicodeScalars {
            guard letter.value >= letterA.value, letter.value < letterA.value + 26,
                let scalar = Unicode.Scalar(regionalIndicatorA + letter.value - letterA.value)
            else { return nil }
            flag.unicodeScalars.append(scalar)
        }
        return flag.unicodeScalars.count == 2 ? flag : nil
    }

    var accessibilityIdForSecureLabel: AccessibilityIdentifier {
        switch tunnelStatus.state {
        case .connected:
            .connectionStatusConnectedLabel
        case .connecting:
            .connectionStatusConnectingLabel
        default:
            .connectionStatusNotConnectedLabel
        }
    }

    var localizedAccessibilityLabelForSecureLabel: LocalizedStringKey {
        let localizedLocations = { (tunnelInfo: SelectedRelays) in
            let country = NSLocalizedString(tunnelInfo.exit.location.country, comment: "")
            let city = NSLocalizedString(tunnelInfo.exit.location.city, comment: "")
            return "\(city), \(country)"
        }

        switch tunnelStatus.state {
        case .disconnected, .waitingForConnectivity, .disconnecting, .pendingReconnect, .error:
            return localizedTitleForSecureLabel
        case let .connected(tunnelInfo, _, _):
            let location = localizedLocations(tunnelInfo)
            return LocalizedStringKey("Connected to \(location)")
        case let .connecting(tunnelInfo, _, _):
            if let tunnelInfo {
                let location = localizedLocations(tunnelInfo)
                return LocalizedStringKey("Connecting to \(location)")
            } else {
                return localizedTitleForSecureLabel
            }
        case let .reconnecting(tunnelInfo, _, _), let .negotiatingEphemeralPeer(tunnelInfo, _, _, _):
            let location = localizedLocations(tunnelInfo)
            return LocalizedStringKey("Reconnecting to \(location)")
        }
    }

    var localizedTitleForSelectLocationButton: LocalizedStringKey {
        switch tunnelStatus.state {
        case .disconnecting, .pendingReconnect, .disconnected, .waitingForConnectivity(.noNetwork):
            LocalizedStringKey(connectionName ?? "Select location")
        case .connecting, .connected, .reconnecting, .waitingForConnectivity(.noConnection),
            .negotiatingEphemeralPeer, .error:
            LocalizedStringKey("Switch location")
        }
    }

    var actionButton: TunnelActionButton {
        switch tunnelStatus.state {
        case .disconnected, .disconnecting(.nothing), .waitingForConnectivity(.noNetwork):
            .connect
        case .connecting, .pendingReconnect, .disconnecting(.reconnect), .waitingForConnectivity(.noConnection),
            .negotiatingEphemeralPeer:
            .cancel
        case .connected, .reconnecting, .error:
            .disconnect
        }
    }

    var titleForCountryAndCity: LocalizedStringKey? {
        guard let tunnelRelays = tunnelStatus.state.relays else {
            return nil
        }

        let country = NSLocalizedString(tunnelRelays.exit.location.country, comment: "")
        let city = NSLocalizedString(tunnelRelays.exit.location.city, comment: "")

        return LocalizedStringKey("\(country), \(city)")
    }

    var titleForServer: LocalizedStringKey? {
        guard let tunnelRelays = tunnelStatus.state.relays else {
            return nil
        }

        let exitName = tunnelRelays.exit.hostname
        let entryName = tunnelRelays.entry?.hostname

        return if let entryName {
            LocalizedStringKey("\(exitName) via \(entryName)")
        } else {
            "\(exitName)"
        }
    }

    var accessibilityLabelForServer: String? {
        guard let tunnelRelays = tunnelStatus.state.relays else {
            return nil
        }

        let exitName = tunnelRelays.exit.hostname
        if let entryName = tunnelRelays.entry?.hostname {
            return String(
                format: NSLocalizedString("Server hostnames: %@ via %@", comment: ""),
                exitName,
                entryName
            )
        } else {
            return String(
                format: NSLocalizedString("Server hostname: %@", comment: ""),
                exitName
            )
        }
    }

    var inAddress: String? {
        guard let tunnelRelays = tunnelStatus.state.relays else {
            return nil
        }

        let observedTunnelState = tunnelStatus.observedState

        var portAndTransport = ""
        if let connectionState = observedTunnelState.connectionState {
            let inPort = connectionState.remotePort
            let protocolLayer = connectionState.transportLayer.name
            portAndTransport = ":\(inPort) \(protocolLayer)"
        }

        guard
            let address = tunnelRelays.entry?.endpoint.socketAddress.ip
                ?? tunnelStatus.state.relays?.exit.endpoint.socketAddress.ip
        else {
            return nil
        }

        return "\(address)\(portAndTransport)"
    }

    // The transport is always QUIC on the Warren network. Surfaced once the
    // tunnel is establishing or up (desktop parity: the connection-details
    // panel shows the tunnel protocol), nil otherwise.
    var tunnelProtocolName: String? {
        showsConnectionDetails ? "QUIC" : nil
    }

    // Two hops = an entry relay is present in addition to the exit. The
    // Warren fleet is multi-hop only, but a 1-hop circuit collapses onto a
    // single node (no entry), so the presence of an entry hop is the tell.
    var isMultihop: Bool {
        tunnelStatus.state.relays?.entry != nil
    }

    // "Out" fallback. Warren runs no am.i-style egress conncheck (the exit
    // IP is redacted for multi-hop), so the exit country/city the daemon
    // already provides identifies where traffic leaves (desktop
    // `formatExitLocation`). nil until a relay selection exists.
    var outExitLocation: String? {
        guard let tunnelRelays = tunnelStatus.state.relays else {
            return nil
        }

        let country = NSLocalizedString(tunnelRelays.exit.location.country, comment: "")
        let city = NSLocalizedString(tunnelRelays.exit.location.city, comment: "")

        return "\(city), \(country)"
    }
}
