//
//  WarrenTunnelStatisticsView.swift
//  WarrenVPN
//
//  Created by Warren on 2026-05-22 (C.6 follow-up).
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  Read-only display of the current Warren tunnel session statistics.
//  Pulls counters from `WarrenQuinnAdapter.status()` via App Group
//  UserDefaults bridge (PacketTunnel extension writes ; main app
//  reads). Useful for support diagnostics + power-user observability.
//
//  Snapshots refresh on every `onAppear` ; no continuous polling so
//  the main app stays cheap on battery.
//

import SwiftUI

public struct WarrenTunnelStatistics: Equatable {
    public let stateLabel: String
    public let bytesIn: UInt64
    public let bytesOut: UInt64
    public let connectedDurationSeconds: UInt64?
    public let failoverCount: UInt32

    public init(
        stateLabel: String,
        bytesIn: UInt64,
        bytesOut: UInt64,
        connectedDurationSeconds: UInt64?,
        failoverCount: UInt32
    ) {
        self.stateLabel = stateLabel
        self.bytesIn = bytesIn
        self.bytesOut = bytesOut
        self.connectedDurationSeconds = connectedDurationSeconds
        self.failoverCount = failoverCount
    }
}

public struct WarrenTunnelStatisticsView: View {
    /// Either a static snapshot (for previews + initial render) or a
    /// closure that re-fetches the snapshot from App Group
    /// `UserDefaults` every 2 s. The `TimelineView`-driven mode lets
    /// the view stay in sync with the producer side
    /// (`WarrenQuinnTunnelImplementation.startStatsBroadcastTask`)
    /// without coupling the view to a Combine publisher.
    private enum Source {
        case fixed(WarrenTunnelStatistics)
        case live(() -> WarrenTunnelStatistics)
    }

    private let source: Source

    /// Build from a static snapshot. Used for previews + unit-test
    /// rendering paths.
    public init(stats: WarrenTunnelStatistics) {
        self.source = .fixed(stats)
    }

    /// Build from a closure that re-fetches the snapshot on each tick.
    /// Used by the production Settings VC so the view follows the
    /// PacketTunnel extension's broadcast cadence.
    public init(fetch: @escaping () -> WarrenTunnelStatistics) {
        self.source = .live(fetch)
    }

    public var body: some View {
        switch source {
        case .fixed(let snapshot):
            content(for: snapshot)
        case .live(let fetch):
            TimelineView(.periodic(from: .now, by: 2)) { _ in
                content(for: fetch())
            }
        }
    }

    @ViewBuilder
    private func content(for stats: WarrenTunnelStatistics) -> some View {
        VStack(alignment: .leading, spacing: 14) {
            Text(
                String(
                    localized: "Tunnel statistics",
                    table: "Settings",
                    comment: "Title of the tunnel session statistics view"
                )
            )
            .font(.mullvadBig)
            .foregroundColor(.white)

            row(
                label: String(localized: "Status", table: "Settings", comment: "Current tunnel state row label"),
                value: stats.stateLabel
            )

            if let duration = stats.connectedDurationSeconds {
                row(
                    label: String(localized: "Connected for", table: "Settings", comment: ""),
                    value: Self.formatDuration(seconds: duration)
                )
            }

            row(
                label: String(localized: "Data received", table: "Settings", comment: ""),
                value: Self.formatBytes(stats.bytesIn)
            )

            row(
                label: String(localized: "Data sent", table: "Settings", comment: ""),
                value: Self.formatBytes(stats.bytesOut)
            )

            row(
                label: String(localized: "Exit relay switches", table: "Settings", comment: ""),
                value: "\(stats.failoverCount)"
            )

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

    /// `1234567` → `"1.2 MB"`. Uses `ByteCountFormatter` so the unit
    /// suffix is localized.
    static func formatBytes(_ value: UInt64) -> String {
        let formatter = ByteCountFormatter()
        formatter.countStyle = .binary
        formatter.allowedUnits = [.useKB, .useMB, .useGB, .useTB]
        formatter.includesUnit = true
        return formatter.string(fromByteCount: Int64(min(value, UInt64(Int64.max))))
    }

    /// `3725` → `"01:02:05"`. Stable monospaced format ; if the
    /// session is < 1 hour shows `MM:SS` only.
    static func formatDuration(seconds: UInt64) -> String {
        let hours = seconds / 3600
        let minutes = (seconds % 3600) / 60
        let secs = seconds % 60
        if hours > 0 {
            return String(format: "%02llu:%02llu:%02llu", hours, minutes, secs)
        } else {
            return String(format: "%02llu:%02llu", minutes, secs)
        }
    }
}

#if DEBUG
#Preview {
    WarrenTunnelStatisticsView(
        stats: WarrenTunnelStatistics(
            stateLabel: "Connected",
            bytesIn: 1_234_567,
            bytesOut: 89_012,
            connectedDurationSeconds: 3725,
            failoverCount: 2
        )
    )
}
#endif
