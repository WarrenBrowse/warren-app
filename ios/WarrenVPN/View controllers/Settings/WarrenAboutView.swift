//
//  WarrenAboutView.swift
//  WarrenVPN
//
//  Created by Warren on 2026-05-22.
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  Settings → "About Warren". Single-screen pointer-board with the
//  marketing site link, the privacy policy, the licences screen and a
//  short version banner.
//

import SwiftUI
import WarrenRustRuntime

public struct WarrenAboutView: View {
    public let appVersion: String
    public let buildNumber: String
    /// The product row this build was compiled for.
    public let anchors: WarrenProductAnchors

    @State private var showsLicenses = false

    public init(
        appVersion: String,
        buildNumber: String,
        anchors: WarrenProductAnchors = .current
    ) {
        self.appVersion = appVersion
        self.buildNumber = buildNumber
        self.anchors = anchors
    }

    /// The title of the screen a user opens to check what they are running. It
    /// is the environment's own display name: a beta install calling itself
    /// "Warren VPN" here answers the question wrongly, on the one screen asked
    /// to answer it.
    public var productName: String { anchors.displayName }

    public var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                Text(productName)
                    .font(.warrenBig)
                    .foregroundColor(.white)

                Text("v\(appVersion) (build \(buildNumber))")
                    .font(.jetBrainsMono(.body))
                    .foregroundColor(.white.opacity(0.6))

                Text(
                    String(
                        localized: "Warren is a wallet-authenticated, censorship-resistant VPN. Your account is a 12-word phrase you alone control - no email, no payment processor account, no centralised identity.",
                        table: "Settings",
                        comment: "Body explaining Warren's differentiator"
                    )
                )
                .font(.instrumentSans(.body))
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
                label: String(localized: "Community forum", table: "Settings", comment: "Opens the Warren community forum in Safari"),
                url: "https://forum.warrenbrowse.com",
                icon: "bubble.left.and.bubble.right"
            )
            linkRow(
                label: String(localized: "Visit warren.ro", table: "Settings", comment: "About → marketing site link"),
                url: "https://warren.ro",
                icon: "safari"
            )
            linkRow(
                label: String(localized: "Privacy policy", table: "Settings", comment: ""),
                url: "https://warren.ro/confidentialite",
                icon: "lock.shield"
            )
            linkRow(
                label: String(localized: "Terms of service", table: "Settings", comment: ""),
                url: "https://warren.ro/cgu",
                icon: "doc.text"
            )
            Button(action: { showsLicenses = true }) {
                rowLabel(
                    label: WarrenLicensesView.title,
                    icon: "chevron.left.forwardslash.chevron.right",
                    trailingIcon: "chevron.right"
                )
            }
            linkRow(
                label: String(localized: "Send feedback", table: "Settings", comment: "Opens mail composer to support@"),
                url: "mailto:support@warrenbrowse.com?subject=Warren%20iOS%20feedback",
                icon: "envelope"
            )
        }
        .sheet(isPresented: $showsLicenses) {
            WarrenLicensesView(onClose: { showsLicenses = false })
        }
    }

    private func rowLabel(label: String, icon: String, trailingIcon: String) -> some View {
        HStack {
            Image(systemName: icon)
                .foregroundColor(.Warren.yellow)
            Text(label)
                .font(.warrenSmallSemiBold)
                .foregroundColor(.white)
            Spacer()
            Image(systemName: trailingIcon)
                .foregroundColor(.white.opacity(0.4))
                .font(.caption)
        }
        .padding(12)
        .background(
            RoundedRectangle(cornerRadius: 8)
                .fill(Color.Warren.surface)
        )
    }

    @ViewBuilder
    private func linkRow(label: String, url: String, icon: String) -> some View {
        Button(action: { Self.openURL(url) }) {
            HStack {
                Image(systemName: icon)
                    .foregroundColor(.Warren.yellow)
                Text(label)
                    .font(.warrenSmallSemiBold)
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

/// The licences of the app and its parts: the GPL the app is distributed
/// under, in full, the credit to the project it derives from, the licence of
/// the tunnel engine, and the source of the code this build was made from.
struct WarrenLicensesView: View {
    let onClose: () -> Void

    static var title: String {
        String(localized: "Licenses", table: "Settings", comment: "Title of the view listing the licences of the app and its parts")
    }

    static var paragraphs: [String] {
        [
            String(
                localized: "This app is free software: you can share it and change it under the terms of the GNU General Public License, version 3 or any later version. The full text is below.",
                table: "Settings",
                comment: "First paragraph of the licences view"
            ),
            String(
                localized: "It is based on the Mullvad VPN app, copyright Mullvad VPN AB, released under the same license.",
                table: "Settings",
                comment: "Credit to the project the app is derived from"
            ),
            String(
                localized: "The tunnel engine, warrenguard, is released under the GNU Affero General Public License, version 3.",
                table: "Settings",
                comment: "Licence of the separate tunnel engine the app runs"
            ),
        ]
    }

    static var sourceLinks: [(label: String, url: String)] {
        [
            (
                String(
                    localized: "Source code of this version",
                    table: "Settings",
                    comment: "Link to the source code of the exact version that is installed"
                ),
                WarrenSourceLicense.appSourceURL
            ),
            (
                String(
                    localized: "Source code of the tunnel engine",
                    table: "Settings",
                    comment: "Link to the source code of the tunnel engine of this version"
                ),
                WarrenSourceLicense.engineSourceURL
            ),
        ]
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                HStack {
                    Text(Self.title)
                        .font(.warrenBig)
                        .foregroundColor(.white)
                    Spacer()
                    Button(NSLocalizedString("Close", comment: ""), action: onClose)
                        .foregroundColor(.Warren.yellow)
                }

                ForEach(Self.paragraphs, id: \.self) { paragraph in
                    Text(paragraph)
                        .font(.instrumentSans(.body))
                        .foregroundColor(.white.opacity(0.8))
                }

                ForEach(Self.sourceLinks, id: \.url) { link in
                    Button(action: {
                        if let url = URL(string: link.url) {
                            UIApplication.shared.open(url)
                        }
                    }) {
                        HStack {
                            Text(link.label)
                                .font(.warrenSmallSemiBold)
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
                }

                Text(WarrenSourceLicense.licenseText)
                    .font(.system(size: 12))
                    .foregroundColor(.white.opacity(0.6))
                    .textSelection(.enabled)
            }
            .padding(16)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .background(Color.Warren.navy)
    }
}
