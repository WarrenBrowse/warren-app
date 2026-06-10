//
//  WarrenNatPmpSettingsView.swift
//  WarrenVPN
//
//  Created by Warren on 2026-05-21.
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  C.6 scaffold - NAT-PMP port-forwarding settings.
//  Warren's signature differentiator vs Mullvad/IVPN abandonment
//  (cf. memory `warren_m4h_f_delivered`). Default OFF: avoids
//  unexpected port exposure ; users opt in when they need it (e.g.
//  qBittorrent torrenting).
//
//  Reads forwarded-port state from App Group `UserDefaults` keys
//  written by the PacketTunnel extension (cf. C.4 design doc §2.3
//  broadcastEvent flow).
//

import SwiftUI

public struct WarrenNatPmpSettingsView: View {
    @State private var isEnabled: Bool = false
    @State private var forwardedPort: UInt16? = nil
    @State private var lifetimeRemaining: TimeInterval? = nil

    public init() {}

    public var body: some View {
        Form {
            Section {
                Toggle(String(localized: "Enable port forwarding", table: "Settings"), isOn: $isEnabled)
                    .tint(.Warren.yellow)
            } footer: {
                Text(String(localized: "Requests an external port from the Warren exit relay via NAT-PMP so peer-to-peer apps (BitTorrent, video calls, self-hosted services) can receive incoming connections.", table: "Settings"))
                    .font(.warrenMicro)
            }

            if isEnabled {
                Section(String(localized: "Forwarded port", table: "Settings")) {
                    portStatusRow
                    if let lifetime = lifetimeRemaining {
                        HStack {
                            Text(String(localized: "Renews in", table: "Settings"))
                                .foregroundColor(.white.opacity(0.7))
                            Spacer()
                            Text(formatLifetime(lifetime))
                                .font(.warrenSmallSemiBold.monospacedDigit())
                                .foregroundColor(.white)
                        }
                    }
                }
            }
        }
        .navigationTitle(String(localized: "Port forwarding", table: "Settings"))
        .onAppear(perform: refreshFromAppGroup)
        .onChange(of: isEnabled) { _, newValue in
            // TODO C.6: persist enabled flag to UserDefaults ; trigger
            // PacketTunnel extension to request/release NAT-PMP mapping
            // via the warren_natpmp_ffi callback path.
            _ = newValue
        }
    }

    @ViewBuilder
    private var portStatusRow: some View {
        HStack {
            Text(String(localized: "External port", table: "Settings"))
                .foregroundColor(.white.opacity(0.7))
            Spacer()
            if let port = forwardedPort {
                Text("\(port)")
                    .font(.warrenSmallSemiBold.monospacedDigit())
                    .foregroundColor(.Warren.yellow)
            } else {
                ProgressView()
                    .tint(.Warren.yellow)
            }
        }
    }

    private func refreshFromAppGroup() {
        // TODO C.6: read from UserDefaults(suiteName:
        // ApplicationConfiguration.securityGroupIdentifier).
        // Keys: WarrenTunnel.natPmpExternalPort (UInt16),
        // WarrenTunnel.natPmpLifetimeRemainingSeconds (Double).
        // For scaffold, leaves the placeholder state.
    }

    private func formatLifetime(_ seconds: TimeInterval) -> String {
        let formatter = DateComponentsFormatter()
        formatter.allowedUnits = [.hour, .minute, .second]
        formatter.zeroFormattingBehavior = .pad
        return formatter.string(from: seconds) ?? "--:--"
    }
}
