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
//  The toggle persists to `LatestTunnelSettings.natPmp`; the tunnel
//  reconnects on the change (TunnelSettingsStrategy) and the PacketTunnel
//  extension requests the mapping through the in-tunnel NAT-PMP client
//  (cf. warren-ios `maybe_spawn_nat_pmp`). Live mapping state arrives
//  back through App Group `UserDefaults` keys written by
//  `WarrenQuinnTunnelImplementation.broadcastEvent`.
//

import SwiftUI
import WarrenSettings

/// Snapshot of the NAT-PMP mapping surface broadcast by the tunnel
/// extension. `status` is nil while no request has resolved yet.
struct WarrenNatPmpSnapshot: Equatable {
    var status: String?
    var externalPort: Int?
    var mappedAt: Date?
    var lifetimeSeconds: Int?

    static func read(fromSuite suiteName: String?) -> WarrenNatPmpSnapshot {
        guard let suiteName, let defaults = UserDefaults(suiteName: suiteName) else {
            return WarrenNatPmpSnapshot()
        }
        let port = defaults.object(forKey: WarrenAppGroupKey.natPmpExternalPort.rawValue) as? Int
        return WarrenNatPmpSnapshot(
            status: defaults.string(forKey: WarrenAppGroupKey.natPmpStatus.rawValue),
            externalPort: port,
            mappedAt: defaults.object(forKey: WarrenAppGroupKey.natPmpMappedAt.rawValue) as? Date,
            lifetimeSeconds: defaults.object(forKey: WarrenAppGroupKey.natPmpLifetimeSeconds.rawValue) as? Int
        )
    }
}

@MainActor
final class WarrenNatPmpSettingsViewModel: ObservableObject {
    @Published var isEnabled: Bool {
        didSet {
            guard oldValue != isEnabled else { return }
            // The settings diff makes TunnelSettingsStrategy reconnect the
            // tunnel, so the extension re-reads the flag and starts (or
            // skips) the NAT-PMP refresh loop on the new session.
            tunnelManager?.updateSettings([
                .natPmp(WarrenNatPmpSettings(state: isEnabled ? .on : .off))
            ])
        }
    }

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

    init(tunnelManager: TunnelManager?) {
        self.tunnelManager = tunnelManager
        self.isEnabled = tunnelManager?.settings.natPmp.isEnabled ?? false
    }
}

extension WarrenNatPmpSnapshot {
    init(fromSuite suiteName: String?) {
        self = Self.read(fromSuite: suiteName)
    }
}

public struct WarrenNatPmpSettingsView: View {
    @ObservedObject private var viewModel: WarrenNatPmpSettingsViewModel

    init(viewModel: WarrenNatPmpSettingsViewModel) {
        self.viewModel = viewModel
    }

    public var body: some View {
        Form {
            Section {
                Toggle(String(localized: "Enable port forwarding", table: "Settings"), isOn: $viewModel.isEnabled)
                    .tint(.Warren.yellow)
            } footer: {
                Text(String(localized: "Requests an external port from the Warren exit relay via NAT-PMP so peer-to-peer apps (BitTorrent, video calls, self-hosted services) can receive incoming connections.", table: "Settings"))
                    .font(.warrenMicro)
            }

            if viewModel.isEnabled {
                Section(String(localized: "Forwarded port", table: "Settings")) {
                    // 1 s cadence keeps the renew countdown honest without a
                    // Combine pipeline; the view only exists while on screen.
                    TimelineView(.periodic(from: .now, by: 1)) { context in
                        statusRows(snapshot: viewModel.snapshot(), now: context.date)
                    }
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

    /// Live status rows: requesting spinner until the first grant, the
    /// granted port + renew countdown while open, a generic failure row
    /// otherwise. The extension never persists the failure category
    /// (no-log), so there is nothing more detailed to show.
    @ViewBuilder
    private func statusRows(snapshot: WarrenNatPmpSnapshot, now: Date) -> some View {
        if !viewModel.tunnelIsSecured {
            Text(String(localized: "Connect to the VPN to request a port.", table: "Settings"))
                .font(.warrenMicro)
                .foregroundColor(.white.opacity(0.7))
        } else {
            switch snapshot.status {
            case "open":
                HStack {
                    Text(String(localized: "External port", table: "Settings"))
                        .foregroundColor(.white.opacity(0.7))
                    Spacer()
                    Text(snapshot.externalPort.map { "\($0)" } ?? "-")
                        .font(.warrenSmallSemiBold.monospacedDigit())
                        .foregroundColor(.Warren.yellow)
                }
                if let remaining = renewCountdown(snapshot: snapshot, now: now) {
                    HStack {
                        Text(String(localized: "Renews in", table: "Settings"))
                            .foregroundColor(.white.opacity(0.7))
                        Spacer()
                        Text(Self.formatLifetime(remaining))
                            .font(.warrenSmallSemiBold.monospacedDigit())
                            .foregroundColor(.white)
                    }
                }
            case "failed":
                Text(String(localized: "Port request failed. Warren retries automatically; reconnect to force a new request.", table: "Settings"))
                    .font(.warrenMicro)
                    .foregroundColor(.white.opacity(0.7))
            default:
                HStack {
                    Text(String(localized: "External port", table: "Settings"))
                        .foregroundColor(.white.opacity(0.7))
                    Spacer()
                    ProgressView()
                        .tint(.Warren.yellow)
                }
            }
        }
    }

    /// Seconds until the tunnel's refresh loop renews the mapping. The
    /// client renews at half the granted lifetime (RFC 6886 practice),
    /// clamped at zero while a renewal is in flight.
    private func renewCountdown(snapshot: WarrenNatPmpSnapshot, now: Date) -> TimeInterval? {
        guard let mappedAt = snapshot.mappedAt, let lifetime = snapshot.lifetimeSeconds else {
            return nil
        }
        let renewAt = mappedAt.addingTimeInterval(TimeInterval(lifetime) / 2)
        return max(0, renewAt.timeIntervalSince(now))
    }

    private static func formatLifetime(_ seconds: TimeInterval) -> String {
        let formatter = DateComponentsFormatter()
        formatter.allowedUnits = [.hour, .minute, .second]
        formatter.zeroFormattingBehavior = .pad
        return formatter.string(from: seconds) ?? "--:--"
    }
}
