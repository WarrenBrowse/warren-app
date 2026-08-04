//
//  ListItem.swift
//  MullvadVPN
//
//  Created by Jon Petersson on 2026-03-19.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import SwiftUI

struct ListItem<StatusIndicator: View>: View {
    let title: String
    var subtitle: String?
    var level: Int = 0
    var selected: Bool = false
    @ViewBuilder var statusIndicator: () -> StatusIndicator?

    var body: some View {
        HStack {
            statusIndicator()
                .frame(width: 24, height: 24)

            VStack(alignment: .leading) {
                Text(title)
                    .font(.warrenSmallSemiBold)
                    .foregroundStyle(selected ? Color.warrenSuccessColor : Color.warrenTextPrimary)
                if let subtitle {
                    Text(subtitle)
                        .font(.warrenMiniSemiBold)
                        .foregroundStyle(Color.warrenTextPrimary.opacity(0.6))
                }
            }

            Spacer()
        }
        .padding(.vertical, 8)
        .padding(.leading, CGFloat(16 * (level + 1)))
        .padding(.trailing, 16)
        .frame(minHeight: UIMetrics.LocationList.cellMinHeight)
    }
}
