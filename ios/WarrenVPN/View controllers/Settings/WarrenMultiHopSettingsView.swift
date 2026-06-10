//
//  WarrenMultiHopSettingsView.swift
//  WarrenVPN
//
//  Created by Warren on 2026-05-21.
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  Multi-hop settings: toggle plus entry/exit country pickers. Persists
//  to the `UserDefaults` keys read by `SettingsReader` in the
//  PacketTunnel extension.
//

import SwiftUI

/// Multi-hop tunnel settings: toggle ON/OFF plus entry/exit country
/// pickers. Default OFF (UX overhead, opt-in per brief decision §C.6).
///
/// Data flow (per `.planning/c4-packet-tunnel-provider-quinn-design.md` §4):
/// ```
/// UI toggle -> UserDefaults.WarrenSettings.multiHopEnabled
///           -> SettingsReader in PacketTunnel reads on next start
///           -> WarrenTunnelConfig.multiHopRelay populated
///           -> WarrenQuinnAdapter.start(config:)
/// ```
public struct WarrenMultiHopSettingsView: View {
    @State private var isEnabled: Bool = false
    @State private var entryCountryCode: String = "SE"
    @State private var exitCountryCode: String = "DE"

    /// List of relay country codes. Production wires to
    /// `warren_multihop_ffi::list_relays(...)` via FFI.
    /// Scaffold: hardcoded subset.
    private static let availableCountries: [(code: String, name: String)] = [
        ("SE", "Sweden"),
        ("DE", "Germany"),
        ("FR", "France"),
        ("NL", "Netherlands"),
        ("CH", "Switzerland"),
        ("US", "United States"),
    ]

    public init() {}

    public var body: some View {
        Form {
            Section {
                Toggle(String(localized: "Enable multi-hop", table: "Settings"), isOn: $isEnabled)
                    .tint(.Warren.yellow)
            } footer: {
                Text(String(localized: "Route traffic through an entry relay before the exit relay. Adds latency (~30-50 ms) and bandwidth overhead. OFF by default.", table: "Settings"))
                    .font(.warrenMicro)
            }

            if isEnabled {
                Section(String(localized: "Entry relay", table: "Settings")) {
                    Picker(String(localized: "Country", table: "Settings"), selection: $entryCountryCode) {
                        ForEach(Self.availableCountries, id: \.code) { country in
                            Text(country.name).tag(country.code)
                        }
                    }
                }
                Section {
                    Picker(String(localized: "Country", table: "Settings"), selection: $exitCountryCode) {
                        ForEach(Self.availableCountries.filter { $0.code != entryCountryCode }, id: \.code) { country in
                            Text(country.name).tag(country.code)
                        }
                    }
                } header: {
                    Text(String(localized: "Exit relay", table: "Settings"))
                } footer: {
                    Text(String(localized: "The exit relay determines your apparent location to websites.", table: "Settings"))
                        .font(.warrenMicro)
                }
            }
        }
        .navigationTitle(String(localized: "Multi-hop", table: "Settings"))
        .onChange(of: isEnabled) { _, newValue in
            // TODO C.6: persist to UserDefaults via SettingsReader / WarrenSettings.
            // Trigger tunnel reconnect via TunnelManager IPC.
            _ = newValue
        }
        .onChange(of: entryCountryCode) { _, newValue in
            // TODO C.6: persist to UserDefaults.
            _ = newValue
        }
        .onChange(of: exitCountryCode) { _, newValue in
            // TODO C.6: persist to UserDefaults.
            _ = newValue
        }
    }
}
