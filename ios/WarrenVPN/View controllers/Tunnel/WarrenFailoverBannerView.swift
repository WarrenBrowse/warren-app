//
//  WarrenFailoverBannerView.swift
//  WarrenVPN
//
//  Created by Warren on 2026-05-21.
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  C.6 scaffold - multi-exit failover banner. Surfaces a transient
//  notification when `WarrenQuinnAdapter` reported a failover to an
//  alternate exit relay (cf. memory `warren_session_b_delivered` M5.B.2
//  `select_failover_alternative_for_attempt`). Consumes App Group
//  UserDefaults keys written by the PacketTunnel extension's
//  `broadcastEvent` (cf. `.planning/c4-packet-tunnel-provider-quinn-
//  design.md` §2.3).
//

import SwiftUI

public struct WarrenFailoverBannerInfo: Equatable {
    public let country: String
    public let occurredAt: Date
}

public struct WarrenFailoverBannerView: View {
    public let info: WarrenFailoverBannerInfo
    public var onDismiss: () -> Void

    public init(info: WarrenFailoverBannerInfo, onDismiss: @escaping () -> Void) {
        self.info = info
        self.onDismiss = onDismiss
    }

    public var body: some View {
        HStack(spacing: 12) {
            Image(systemName: "arrow.triangle.swap")
                .foregroundColor(.Warren.yellow)
                .font(.title3)
            VStack(alignment: .leading, spacing: 2) {
                Text(String(localized: "Switched exit relay", table: "Settings"))
                    .font(.warrenSmallSemiBold)
                    .foregroundColor(.white)
                Text(
                    String(
                        format: String(localized: "Reconnected through %@. Your traffic stayed protected.", table: "Settings"),
                        info.country
                    )
                )
                .font(.warrenMicro)
                .foregroundColor(.white.opacity(0.7))
            }
            Spacer()
            Button(action: onDismiss) {
                Image(systemName: "xmark")
                    .foregroundColor(.white.opacity(0.6))
            }
            .accessibilityLabel(String(localized: "Dismiss", table: "Settings"))
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
        .accessibilityLabel(
            String(
                format: String(localized: "VPN reconnected through %@", table: "Settings"),
                info.country
            )
        )
    }
}

#if DEBUG
#Preview {
    WarrenFailoverBannerView(
        info: WarrenFailoverBannerInfo(country: "Switzerland", occurredAt: Date()),
        onDismiss: {}
    )
    .padding()
    .background(Color.Warren.navy)
}
#endif
