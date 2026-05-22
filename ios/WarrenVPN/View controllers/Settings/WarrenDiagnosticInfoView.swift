//
//  WarrenDiagnosticInfoView.swift
//  WarrenVPN
//
//  Created by Warren on 2026-05-22.
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  Single-screen support ticket payload : app version + build number +
//  wallet pubkey (short hex) + tunnel session counters. Designed to
//  fit in a single screenshot for fast support diagnostics. No secrets
//  (mnemonic / seed) are surfaced ; pubkey is non-secret per Ed25519.
//

import SwiftUI

public struct WarrenDiagnosticInfo: Equatable {
    public let appVersion: String
    public let buildNumber: String
    public let walletPubkeyShortHex: String?
    public let tunnelStats: WarrenTunnelStatistics

    public init(
        appVersion: String,
        buildNumber: String,
        walletPubkeyShortHex: String?,
        tunnelStats: WarrenTunnelStatistics
    ) {
        self.appVersion = appVersion
        self.buildNumber = buildNumber
        self.walletPubkeyShortHex = walletPubkeyShortHex
        self.tunnelStats = tunnelStats
    }
}

public struct WarrenDiagnosticInfoView: View {
    public let info: WarrenDiagnosticInfo
    @State private var didCopy = false

    public init(info: WarrenDiagnosticInfo) {
        self.info = info
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text(
                String(
                    localized: "Diagnostic info",
                    table: "Settings",
                    comment: "Title of the support-friendly diagnostic summary"
                )
            )
            .font(.mullvadBig)
            .foregroundColor(.white)

            Text(
                String(
                    localized: "Take a screenshot and attach it to your support request. This screen contains no secrets — your recovery phrase and wallet seed are never displayed here.",
                    table: "Settings",
                    comment: "Body explaining the screen is screenshot-safe"
                )
            )
            .font(.mullvadMicro)
            .foregroundColor(.white.opacity(0.7))

            VStack(spacing: 8) {
                row(label: "Warren VPN", value: "v\(info.appVersion) (build \(info.buildNumber))")
                if let pubkey = info.walletPubkeyShortHex {
                    row(label: "Wallet ID", value: pubkey)
                }
                row(label: "Status", value: info.tunnelStats.stateLabel)
                if let duration = info.tunnelStats.connectedDurationSeconds {
                    row(
                        label: "Connected for",
                        value: WarrenTunnelStatisticsView.formatDuration(seconds: duration)
                    )
                }
                row(label: "Data in", value: WarrenTunnelStatisticsView.formatBytes(info.tunnelStats.bytesIn))
                row(label: "Data out", value: WarrenTunnelStatisticsView.formatBytes(info.tunnelStats.bytesOut))
                row(label: "Failovers", value: "\(info.tunnelStats.failoverCount)")
            }

            Button(action: copySummary) {
                HStack {
                    Image(systemName: didCopy ? "checkmark" : "doc.on.doc")
                    Text(
                        didCopy
                        ? String(localized: "Copied", table: "Settings", comment: "")
                        : String(
                            localized: "Copy as text",
                            table: "Settings",
                            comment: "Diagnostic info copy-to-clipboard button"
                        )
                    )
                }
                .font(.mullvadSmallSemiBold)
                .foregroundColor(.Warren.navy)
                .padding(.horizontal, 16)
                .padding(.vertical, 10)
                .background(
                    RoundedRectangle(cornerRadius: 8)
                        .fill(Color.Warren.yellow)
                )
            }

            Spacer()
        }
        .padding(16)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Color.Warren.navy)
    }

    private func row(label: String, value: String) -> some View {
        HStack {
            Text(label)
                .font(.mullvadSmallSemiBold)
                .foregroundColor(.white.opacity(0.7))
            Spacer()
            Text(value)
                .font(.system(.body, design: .monospaced))
                .foregroundColor(.white)
                .accessibilityLabel("\(label) : \(value)")
        }
        .padding(12)
        .background(
            RoundedRectangle(cornerRadius: 8)
                .fill(Color.Warren.surface)
        )
    }

    private func copySummary() {
        UIPasteboard.general.string = Self.plainTextSummary(info)
        didCopy = true
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) {
            didCopy = false
        }
    }

    /// Multi-line plain-text rendering of the diagnostic info suitable
    /// for pasting into a support ticket. Public for unit testing.
    public static func plainTextSummary(_ info: WarrenDiagnosticInfo) -> String {
        var lines: [String] = []
        lines.append("Warren VPN v\(info.appVersion) (build \(info.buildNumber))")
        if let pubkey = info.walletPubkeyShortHex {
            lines.append("Wallet ID: \(pubkey)")
        }
        lines.append("Status: \(info.tunnelStats.stateLabel)")
        if let duration = info.tunnelStats.connectedDurationSeconds {
            lines.append("Connected for: \(WarrenTunnelStatisticsView.formatDuration(seconds: duration))")
        }
        lines.append("Data in: \(WarrenTunnelStatisticsView.formatBytes(info.tunnelStats.bytesIn))")
        lines.append("Data out: \(WarrenTunnelStatisticsView.formatBytes(info.tunnelStats.bytesOut))")
        lines.append("Failovers: \(info.tunnelStats.failoverCount)")
        return lines.joined(separator: "\n")
    }
}
