//
//  WarrenObfuscationIndicatorView.swift
//  WarrenVPN
//
//  Created by Warren on 2026-05-21.
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  Scaffold for C.6 — M4.0 obfuscation indicator banner.
//  Surfaces the always-on HTTP/3 mimicry in connection-details so
//  users understand that Warren traffic is indistinguishable from
//  regular HTTPS by default (no toggle, always-on baseline). NOT yet
//  wired into the Xcode project.
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
