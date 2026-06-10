//
//  WarrenDaitaSettingsView.swift
//  WarrenVPN
//
//  Created by Warren on 2026-05-21.
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  C.6 scaffold - DAITA toggle in Settings. DAITA defaults OFF
//  (per memory `warren_daita_doctrine_v1` + Session F instability
//  finding resolved in Session G `pump_*_with_daita` production
//  wiring fix). Persists the user choice to App Group UserDefaults
//  consumed by the PacketTunnel `SettingsReader` at next connect.
//

import SwiftUI

/// DAITA defensive traffic shaping toggle.
///
/// DAITA is a Mullvad-derived padding technique that hides the
/// fingerprint of VPN traffic by emitting cover packets according to
/// a Maybenot state machine. Warren wires DAITA through warren-tunnel
/// (cf. memory `warren_session_b_delivered` M5.B.1 + Session G fix).
///
/// Default OFF: trade-off ~10% bandwidth overhead, opt-in.
public struct WarrenDaitaSettingsView: View {
    @State private var isEnabled: Bool = false

    public init() {}

    public var body: some View {
        Form {
            Section {
                Toggle(String(localized: "Enable DAITA", table: "Settings"), isOn: $isEnabled)
                    .tint(.Warren.yellow)
            } footer: {
                Text(String(localized: "Defense Against AI-Guided Traffic Analysis. Emits constant-rate cover traffic to make your VPN sessions indistinguishable to network observers. Adds ~10% bandwidth overhead.", table: "Settings"))
                    .font(.warrenMicro)
            }

            if isEnabled {
                Section {
                    Label(
                        String(localized: "DAITA is experimental and may degrade throughput on high-bandwidth links.", table: "Settings"),
                        systemImage: "exclamationmark.triangle.fill"
                    )
                    .font(.warrenMicro)
                    .foregroundColor(.Warren.yellow)
                    .listRowBackground(Color.Warren.surface)
                }
            }
        }
        .navigationTitle(String(localized: "DAITA", table: "Settings"))
        .onChange(of: isEnabled) { _, newValue in
            // TODO C.6: persist to UserDefaults via SettingsReader / WarrenSettings.
            // Trigger tunnel reconnect via TunnelManager IPC.
            _ = newValue
        }
    }
}
