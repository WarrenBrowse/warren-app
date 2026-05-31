//
//  WarrenObfuscationIndicatorView.swift
//  WarrenVPN
//
//  Created by Warren on 2026-05-21.
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  M4.0 obfuscation indicator banner.
//  Surfaces the always-on HTTP/3 mimicry in connection-details so
//  users understand that Warren traffic is indistinguishable from
//  regular HTTPS by default (no toggle, always-on baseline). Also
//  hosted as the read-only obfuscation settings screen, since the
//  legacy WireGuard obfuscation methods do not apply to Warren tunnels.
//

import SwiftUI

/// Always-on obfuscation banner. M4.0 HTTP/3 mimicry is the Warren
/// baseline (no Mullvad-style bridge/Shadowsocks rotation needed).
/// Shown in the connection-details panel below the relay info.
public struct WarrenObfuscationIndicatorView: View {
    public init() {}

    public var body: some View {
        HStack(spacing: 12) {
            Image(systemName: "checkmark.shield.fill")
                .foregroundColor(.Warren.yellow)
                .font(.title3)
            VStack(alignment: .leading, spacing: 2) {
                Text(String(localized: "HTTP/3 mimicry active", table: "Settings"))
                    .font(.mullvadSmallSemiBold)
                    .foregroundColor(.white)
                Text(String(localized: "Your VPN traffic is indistinguishable from regular HTTPS browsing.", table: "Settings"))
                    .font(.mullvadMicro)
                    .foregroundColor(.white.opacity(0.7))
            }
            Spacer()
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 12)
        .background(
            RoundedRectangle(cornerRadius: 10)
                .fill(Color.Warren.surface)
                .overlay(
                    RoundedRectangle(cornerRadius: 10)
                        .stroke(Color.Warren.yellow.opacity(0.3), lineWidth: 1)
                )
        )
        .accessibilityElement(children: .combine)
        .accessibilityLabel(String(localized: "HTTP/3 mimicry is active. Traffic is indistinguishable from regular HTTPS.", table: "Settings"))
    }
}

/// Read-only obfuscation settings screen for Warren tunnels.
///
/// Warren runs warren-core's QUIC transport, whose M4.0 HTTP/3 mimicry is
/// always-on and not togglable (disabling it would make Warren clients
/// immediately recognisable on the network). The legacy Mullvad obfuscation
/// methods (Shadowsocks, UDP-over-TCP, QUIC, LWO) are WireGuard-only and do
/// not apply to Warren tunnels, so no interactive picker is offered — this
/// mirrors the desktop anti-censorship view when warren_mode is on.
public struct WarrenObfuscationSettingsReadOnlyView: View {
    public init() {}

    public var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            WarrenObfuscationIndicatorView()

            Text(String(
                localized: "Warren tunnels masquerade as standard browser HTTP/3 traffic: ALPN h3, SNI warrenbrowse.com, Initial-packet split, UDP/443. This is not togglable because disabling it would make Warren clients immediately recognisable on the network.",
                table: "Settings"
            ))
            .font(.mullvadSmall)
            .foregroundColor(.white.opacity(0.8))

            Text(String(
                localized: "Legacy obfuscation methods (Shadowsocks, UDP-over-TCP, QUIC, LWO) do not apply to Warren tunnels.",
                table: "Settings"
            ))
            .font(.mullvadSmall)
            .foregroundColor(.white.opacity(0.6))

            Spacer(minLength: 0)
        }
        .padding(16)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
    }
}
