//
//  WarrenAboutView.swift
//  WarrenVPN
//
//  Created by Warren on 2026-05-22.
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  Settings → "About Warren". Single-screen pointer-board with the
//  marketing site link + privacy policy + AGPL-3.0 license note + a
//  short version banner. Designed for legal compliance (App Store
//  privacy disclosure + AGPL source-availability link) and first-time
//  user education ("what makes Warren different from Mullvad ?").
//

import SwiftUI

public struct WarrenAboutView: View {
    public let appVersion: String
    public let buildNumber: String

    public init(appVersion: String, buildNumber: String) {
        self.appVersion = appVersion
        self.buildNumber = buildNumber
    }

    public var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                Text("Warren VPN")
                    .font(.mullvadBig)
                    .foregroundColor(.white)

                Text("v\(appVersion) (build \(buildNumber))")
                    .font(.system(.body, design: .monospaced))
                    .foregroundColor(.white.opacity(0.6))

                Text(
                    String(
                        localized: "Warren is a wallet-authenticated, censorship-resistant VPN. Your account is a 12-word phrase you alone control - no email, no payment processor account, no centralised identity.",
                        table: "Settings",
                        comment: "Body explaining Warren's differentiator"
                    )
                )
                .font(.body)
                .foregroundColor(.white.opacity(0.8))

                linksGroup
                Spacer()
            }
            .padding(16)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .background(Color.Warren.navy)
    }

    @ViewBuilder
    private var linksGroup: some View {
        VStack(spacing: 8) {
            linkRow(
                label: String(localized: "Visit warrenbrowse.com", table: "Settings", comment: "About → marketing site link"),
                url: "https://warrenbrowse.com",
                icon: "safari"
            )
            linkRow(
                label: String(localized: "Privacy policy", table: "Settings", comment: ""),
                url: "https://warrenbrowse.com/privacy",
                icon: "lock.shield"
            )
            linkRow(
                label: String(localized: "Terms of service", table: "Settings", comment: ""),
                url: "https://warrenbrowse.com/terms",
                icon: "doc.text"
            )
            linkRow(
                label: String(localized: "Source code (AGPL-3.0)", table: "Settings", comment: "Repo link"),
                url: "https://github.com/WarrenBrowse/warren-app",
                icon: "chevron.left.forwardslash.chevron.right"
            )
            linkRow(
                label: String(localized: "Send feedback", table: "Settings", comment: "Opens mail composer to support@"),
                url: "mailto:support@warrenbrowse.com?subject=Warren%20iOS%20feedback",
                icon: "envelope"
            )
        }

        Text(
            String(
                localized: "Warren is a fork of Mullvad VPN. Mullvad's upstream code is licensed under GPL-3.0 ; Warren's modifications are under AGPL-3.0.",
                table: "Settings",
                comment: "Bottom legal credit explaining the Mullvad fork relationship + license"
            )
        )
        .font(.mullvadMicro)
        .foregroundColor(.white.opacity(0.5))
        .padding(.top, 12)
    }

    @ViewBuilder
    private func linkRow(label: String, url: String, icon: String) -> some View {
        Button(action: { Self.openURL(url) }) {
            HStack {
                Image(systemName: icon)
                    .foregroundColor(.Warren.yellow)
                Text(label)
                    .font(.mullvadSmallSemiBold)
                    .foregroundColor(.white)
                Spacer()
                Image(systemName: "arrow.up.right")
                    .foregroundColor(.white.opacity(0.4))
                    .font(.caption)
            }
            .padding(12)
            .background(
                RoundedRectangle(cornerRadius: 8)
                    .fill(Color.Warren.surface)
            )
        }
        .accessibilityHint(
            String(
                localized: "Opens %@ in Safari",
                table: "Settings",
                comment: "VoiceOver hint formatted with the URL"
            )
            .replacingOccurrences(of: "%@", with: url)
        )
    }

    private static func openURL(_ urlString: String) {
        guard let url = URL(string: urlString) else { return }
        UIApplication.shared.open(url)
    }
}
