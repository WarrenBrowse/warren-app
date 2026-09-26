//
//  WarrenNatPmpSettingsView.swift
//  WarrenVPN
//
//  Created by Warren on 2026-05-21.
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  NAT-PMP port-forwarding settings. Warren's signature differentiator
//  vs Mullvad/IVPN abandonment of port forwarding. Default OFF: avoids
//  unexpected port exposure ; users opt in when they need it (e.g.
//  qBittorrent torrenting).
//
//  The settings persist to `LatestTunnelSettings.natPmp`; the tunnel
//  reconnects on the change (TunnelSettingsStrategy) and the PacketTunnel
//  extension asks the exit for that mapping through the in-tunnel NAT-PMP
//  client (cf. warren-ios `maybe_spawn_nat_pmp`). Live mapping state arrives
//  back through App Group `UserDefaults` keys written by
//  `WarrenQuinnTunnelImplementation.broadcastEvent`.
//

import SwiftUI
import WarrenRustRuntime
import WarrenSettings

/// Snapshot of the NAT-PMP mapping surface broadcast by the tunnel
/// extension. `status` is nil while no request has resolved yet.
struct WarrenNatPmpSnapshot: Equatable {
    var status: String?
    var externalPort: Int?
    var mappedAt: Date?
    var lifetimeSeconds: Int?
    /// The stable refusal CATEGORY of the last failed request, never a raw
    /// error string. It is what tells a port conflict, which the user can
    /// act on, from every other refusal.
    var failureReason: String?
    var retryAfterSeconds: Int?
    var rateLimitedAt: Date?
    /// What the exit refused the last mapping for as not authorized
    /// (`no_entitlement` or `entitlement_refused`), and when; the tunnel asks
    /// again after `retryAfterSeconds`.
    var refusal: String?
    var refusedAt: Date?

    static func read(fromSuite suiteName: String?) -> WarrenNatPmpSnapshot {
        guard let suiteName, let defaults = UserDefaults(suiteName: suiteName) else {
            return WarrenNatPmpSnapshot()
        }
        let port = defaults.object(forKey: WarrenAppGroupKey.natPmpExternalPort.rawValue) as? Int
        return WarrenNatPmpSnapshot(
            status: defaults.string(forKey: WarrenAppGroupKey.natPmpStatus.rawValue),
            externalPort: port,
            mappedAt: defaults.object(forKey: WarrenAppGroupKey.natPmpMappedAt.rawValue) as? Date,
            lifetimeSeconds: defaults.object(forKey: WarrenAppGroupKey.natPmpLifetimeSeconds.rawValue) as? Int,
            failureReason: defaults.string(forKey: WarrenAppGroupKey.natPmpFailureReason.rawValue),
            retryAfterSeconds: defaults.object(forKey: WarrenAppGroupKey.natPmpRetryAfterSeconds.rawValue) as? Int,
            rateLimitedAt: defaults.object(forKey: WarrenAppGroupKey.natPmpRateLimitedAt.rawValue) as? Date,
            refusal: defaults.string(forKey: WarrenAppGroupKey.natPmpRefusal.rawValue),
            refusedAt: defaults.object(forKey: WarrenAppGroupKey.natPmpRefusedAt.rawValue) as? Date
        )
    }
}

@MainActor
final class WarrenNatPmpSettingsViewModel: ObservableObject {
    @Published var isEnabled: Bool {
        didSet {
            guard oldValue != isEnabled else { return }
            apply()
        }
    }

    @Published var networkProtocol: WarrenNatPmpProtocol {
        didSet {
            guard oldValue != networkProtocol else { return }
            apply()
        }
    }

    @Published var lifetimeSeconds: UInt32 {
        didSet {
            guard oldValue != lifetimeSeconds else { return }
            apply()
        }
    }

    /// The port as typed, committed only when it is one an exit will
    /// consider: a half-typed "6" must not reach the tunnel as a pin.
    @Published var portInput: String

    /// Whether what is in the field right now can be committed.
    var portInputIsValid: Bool {
        WarrenPortForwarding.port(fromInput: portInput) != nil
    }

    private var externalPort: UInt16
    private let tunnelManager: TunnelManager?

    /// Whether a live tunnel session exists for the mapping to ride on.
    /// Mirrors `IncludeAllNetworksSettingsViewModelImpl.tunnelIsSecured`.
    var tunnelIsSecured: Bool {
        guard let tunnelManager else { return false }
        return tunnelManager.tunnelStatus.state != .error(.offline)
            && tunnelManager.tunnelStatus.state.isSecured
    }

    func snapshot() -> WarrenNatPmpSnapshot {
        WarrenNatPmpSnapshot(fromSuite: ApplicationConfiguration.securityGroupIdentifier)
    }

    /// The standing the app last heard, `nil` while nothing is known.
    func standing() -> WarrenAccountStanding? {
        WarrenAccountStandingFeed.current?.standing
    }

    func state(now: Date) -> WarrenPortForwardingState {
        WarrenPortForwarding.state(
            snapshot: snapshot(),
            tunnelIsSecured: tunnelIsSecured,
            now: now
        )
    }

    init(tunnelManager: TunnelManager?) {
        let settings = tunnelManager?.settings.natPmp ?? WarrenNatPmpSettings()
        self.tunnelManager = tunnelManager
        self.isEnabled = settings.isEnabled
        self.networkProtocol = settings.networkProtocol
        self.lifetimeSeconds = settings.lifetimeSeconds
        self.externalPort = settings.externalPort
        self.portInput = WarrenPortForwarding.input(forPort: settings.externalPort)
    }

    /// Commits the typed port, if it is one. Called when the field loses
    /// focus or the user submits, never on every keystroke.
    func commitPort() {
        guard let port = WarrenPortForwarding.port(fromInput: portInput), port != externalPort else {
            return
        }
        externalPort = port
        apply()
    }

    /// Lets the exit pick any free port. One of the two ways out of a port
    /// conflict; editing the field is the third.
    func assignFreePort() {
        portInput = ""
        externalPort = 0
        apply()
    }

    private func apply() {
        // The settings diff makes TunnelSettingsStrategy reconnect the
        // tunnel, so the extension re-reads the mapping and asks the exit for
        // it on the new session.
        tunnelManager?.updateSettings([
            .natPmp(
                WarrenNatPmpSettings(
                    state: isEnabled ? .on : .off,
                    networkProtocol: networkProtocol,
                    externalPort: externalPort,
                    lifetimeSeconds: lifetimeSeconds
                ))
        ])
    }
}

extension WarrenNatPmpSnapshot {
    init(fromSuite suiteName: String?) {
        self = Self.read(fromSuite: suiteName)
    }
}

public struct WarrenNatPmpSettingsView: View {
    @ObservedObject private var viewModel: WarrenNatPmpSettingsViewModel
    /// Opens the abuse-report page. Handed in so the view never reaches for
    /// `UIApplication` itself.
    private let openURL: (URL) -> Void

    init(
        viewModel: WarrenNatPmpSettingsViewModel,
        openURL: @escaping (URL) -> Void = { UIApplication.shared.open($0) }
    ) {
        self.viewModel = viewModel
        self.openURL = openURL
    }

    public var body: some View {
        Form {
            Section {
                Toggle(String(localized: "Enable port forwarding", table: "Settings"), isOn: $viewModel.isEnabled)
                    .tint(.Warren.yellow)
            } footer: {
                VStack(alignment: .leading, spacing: 8) {
                    Text(String(localized: "Requests an external port from the Warren exit relay via NAT-PMP so peer-to-peer apps (BitTorrent, video calls, self-hosted services) can receive incoming connections.", table: "Settings"))
                    abuseNotice
                }
                .font(.warrenMicro)
            }

            if let standing = viewModel.standing(), Self.hasStandingToShow(standing) {
                Section(String(localized: "Warnings", table: "Settings")) {
                    Self.standingRows(standing)
                }
            }

            if viewModel.isEnabled {
                Section(String(localized: "Forwarded port", table: "Settings")) {
                    // 1 s cadence keeps the countdowns honest without a
                    // Combine pipeline; the view only exists while on screen.
                    TimelineView(.periodic(from: .now, by: 1)) { context in
                        statusRows(state: viewModel.state(now: context.date))
                    }
                }

                Section(String(localized: "Mapping", table: "Settings")) {
                    Picker(
                        String(localized: "Protocol", table: "Settings"),
                        selection: $viewModel.networkProtocol
                    ) {
                        Text("UDP").tag(WarrenNatPmpProtocol.udp)
                        Text("TCP").tag(WarrenNatPmpProtocol.tcp)
                    }
                    .pickerStyle(.segmented)

                    preferredPortRow

                    Picker(
                        String(localized: "Lease", table: "Settings"),
                        selection: $viewModel.lifetimeSeconds
                    ) {
                        ForEach(WarrenNatPmpSettings.lifetimeChoices, id: \.self) { seconds in
                            Text(Self.lifetimeLabel(seconds)).tag(seconds)
                        }
                    }
                    .pickerStyle(.segmented)
                }
            }
        }
        // The Form paints systemGroupedBackground over the hosting
        // controller's navy; hide it and pin the dark scheme like the
        // sibling Warren settings views (About, Tunnel statistics).
        .scrollContentBackground(.hidden)
        .background(Color.Warren.navy)
        .environment(\.colorScheme, .dark)
        .navigationTitle(String(localized: "Port forwarding", table: "Settings"))
    }

    /// An open port is reachable by anyone, so a third-party abuse report can
    /// reach Warren about it. Stating the rule here, rather than only in the
    /// terms, is what makes the consequence foreseeable to whoever opens the
    /// port.
    @ViewBuilder
    private var abuseNotice: some View {
        Text(String(localized: "A forwarded port is reachable from the internet. If a third party reports abuse coming from it, Warren closes the port and records a warning on the account; three warnings within 90 days revoke access.", table: "Settings"))
            .foregroundColor(.white.opacity(0.7))
        if let url = URL(string: Self.abuseReportsURL) {
            Button(String(localized: "Details, and how to contest a warning", table: "Settings")) {
                openURL(url)
            }
            .font(.warrenMicro)
        }
    }

    private var preferredPortRow: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                Text(String(localized: "Preferred port", table: "Settings"))
                Spacer()
                TextField(
                    String(localized: "Any", table: "Settings"),
                    text: $viewModel.portInput
                )
                .keyboardType(.numberPad)
                .multilineTextAlignment(.trailing)
                .font(.warrenSmallSemiBold.monospacedDigit())
                .onSubmit { viewModel.commitPort() }
            }
            Text(
                viewModel.portInputIsValid
                    ? String(
                        format: String(localized: "Leave empty to let the exit pick. %d to %d otherwise.", table: "Settings"),
                        Int(WarrenNatPmpSettings.portRange.lowerBound),
                        Int(WarrenNatPmpSettings.portRange.upperBound))
                    : String(
                        format: String(localized: "A port is a number from %d to %d.", table: "Settings"),
                        Int(WarrenNatPmpSettings.portRange.lowerBound),
                        Int(WarrenNatPmpSettings.portRange.upperBound))
            )
            .font(.warrenMicro)
            .foregroundColor(viewModel.portInputIsValid ? .white.opacity(0.7) : .Warren.error)
        }
        .onChange(of: viewModel.portInput) { _, _ in
            // Committing on every keystroke would reconnect the tunnel while
            // the number is still being typed.
            guard viewModel.portInputIsValid else { return }
        }
        .onDisappear { viewModel.commitPort() }
    }

    /// Live status rows, one per state the snapshot can be in.
    @ViewBuilder
    private func statusRows(state: WarrenPortForwardingState) -> some View {
        switch state {
        case .noTunnel:
            Text(String(localized: "Connect to the VPN to request a port.", table: "Settings"))
                .font(.warrenMicro)
                .foregroundColor(.white.opacity(0.7))
        case .requesting:
            HStack {
                Text(String(localized: "External port", table: "Settings"))
                    .foregroundColor(.white.opacity(0.7))
                Spacer()
                ProgressView()
                    .tint(.Warren.yellow)
            }
        case let .mapped(port, renewsIn):
            HStack {
                Text(String(localized: "External port", table: "Settings"))
                    .foregroundColor(.white.opacity(0.7))
                Spacer()
                Text("\(port)")
                    .font(.warrenSmallSemiBold.monospacedDigit())
                    .foregroundColor(.Warren.yellow)
            }
            if let renewsIn {
                HStack {
                    Text(String(localized: "Renews in", table: "Settings"))
                        .foregroundColor(.white.opacity(0.7))
                    Spacer()
                    Text(WarrenPortForwarding.countdown(renewsIn))
                        .font(.warrenSmallSemiBold.monospacedDigit())
                        .foregroundColor(.white)
                }
            }
        case let .rateLimited(remaining):
            Text(
                String(
                    format: String(localized: "The exit is not handing out ports right now. Try again in %@.", table: "Settings"),
                    WarrenPortForwarding.countdown(remaining))
            )
            .font(.warrenMicro)
            .foregroundColor(.Warren.error)
        case let .refused(noEntitlement, retryIn):
            Text(
                String(
                    format: noEntitlement
                        ? String(localized: "Status: no entitlement left, retrying in %1$@s", table: "Settings")
                        : String(localized: "Status: refused, retrying in %1$@s", table: "Settings"),
                    String(Int(retryIn.rounded(.up))))
            )
            .font(.warrenMicro)
            .foregroundColor(.white.opacity(0.7))
        case let .failed(portConflict):
            if portConflict {
                Text(String(localized: "That port is already taken on this exit.", table: "Settings"))
                    .font(.warrenMicro)
                    .foregroundColor(.Warren.error)
                Button(String(localized: "Let the exit pick a port", table: "Settings")) {
                    viewModel.assignFreePort()
                }
            } else {
                Text(String(localized: "Port request failed. Warren retries automatically; reconnect to force a new request.", table: "Settings"))
                    .font(.warrenMicro)
                    .foregroundColor(.white.opacity(0.7))
            }
        }
    }

    /// The account's standing (warren-core doc 105): the ban in force with the
    /// day it lapses, every live warning with its case reference, and how to
    /// contest one. The references are shown here and in the strike notice
    /// only, the one place the reader needs them to write to the abuse desk.
    private static func standingRows(_ standing: WarrenAccountStanding) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            if let ban = standing.ban, ban.inForce {
                Text(WarrenAccountStandingText.ban(ban))
                    .foregroundColor(.Warren.error)
            }
            ForEach(Array(standing.strikes.enumerated()), id: \.offset) { index, strike in
                Text(
                    WarrenAccountStandingText.warning(
                        WarrenStrikeNotice(strike: strike, ordinal: index + 1, threshold: standing.threshold)
                    ) + " " + WarrenAccountStandingText.caseReference(strike)
                )
            }
            if !standing.strikes.isEmpty {
                Text(WarrenAccountStandingText.contest())
                    .foregroundColor(.white.opacity(0.7))
            }
        }
        .font(.warrenMicro)
    }

    /// Whether the standing has anything to show: a ban in force or a live
    /// warning.
    static func hasStandingToShow(_ standing: WarrenAccountStanding) -> Bool {
        standing.ban?.inForce == true || !standing.strikes.isEmpty
    }

    /// The page that states the strike rule and the way to contest one, the
    /// same the Android screen links.
    static let abuseReportsURL = WarrenAccountStandingText.reportsURL

    private static func lifetimeLabel(_ seconds: UInt32) -> String {
        let formatter = DateComponentsFormatter()
        formatter.allowedUnits = seconds >= 3600 ? [.hour] : [.minute]
        formatter.unitsStyle = .abbreviated
        return formatter.string(from: TimeInterval(seconds)) ?? "\(seconds)"
    }
}
