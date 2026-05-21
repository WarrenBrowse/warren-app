//
//  WarrenMultiHopSettingsView.swift
//  WarrenVPN
//
//  Created by Warren on 2026-05-21.
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  Scaffold for C.6 — Multi-hop UI parity with desktop M4.H.C.
//  Toggle + entry country picker + exit country picker. Persists
//  selection to `UserDefaults` keys consumed by `SettingsReader` in the
//  PacketTunnel extension. NOT yet wired into the Xcode project.
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
                Toggle("Enable multi-hop", isOn: $isEnabled)
                    .tint(.Warren.yellow)
            } footer: {
                Text("Route traffic through an entry relay before the exit relay. Adds latency (~30-50 ms) and bandwidth overhead. OFF by default.")
                    .font(.caption)
            }

            if isEnabled {
                Section("Entry relay") {
                    Picker("Country", selection: $entryCountryCode) {
                        ForEach(Self.availableCountries, id: \.code) { country in
                            Text(country.name).tag(country.code)
                        }
                    }
                }
                Section("Exit relay") {
                    Picker("Country", selection: $exitCountryCode) {
                        ForEach(Self.availableCountries.filter { $0.code != entryCountryCode }, id: \.code) { country in
                            Text(country.name).tag(country.code)
                        }
                    }
                } footer: {
                    Text("The exit relay determines your apparent location to websites.")
                        .font(.caption)
                }
            }
        }
        .navigationTitle("Multi-hop")
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
