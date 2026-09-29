//
//  FeatureChipView.swift
//  MullvadVPN
//
//  Created by Mojgan on 2024-12-05.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import SwiftUI

/// A feature pill over the scenery: an opaque surface of the theme, like the
/// card below it (desktop `FeatureIndicator`).
struct ChipView: View {
    let item: ChipModel
    let onPress: (() -> Void)?

    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let palette = ConnectSurfacePalette(colorScheme: colorScheme)
        let metrics = ConnectSurfaceMetrics.Chip.self
        let shape = RoundedRectangle(cornerRadius: metrics.radius)

        Button {
            onPress?()
        } label: {
            HStack(spacing: UIMetrics.FeatureIndicators.chipViewIconTextSpacing) {
                if let icon = item.icon {
                    icon
                        .resizable()
                        .frame(width: 14, height: 14)
                }
                Text(item.name)
                    .connectFont(size: metrics.textSize, weight: .semibold, relativeTo: .caption)
                    .lineLimit(1)
                    .foregroundStyle(palette.color(.text))
            }
            .padding(.vertical, metrics.paddingVertical)
            .padding(.horizontal, metrics.paddingHorizontal)
            .background(
                shape
                    .fill(palette.color(.card))
                    .shadow(
                        color: palette.color(.shadowSoft),
                        radius: metrics.shadowBlur / 2,
                        x: 0,
                        y: metrics.shadowOffsetY
                    )
            )
            .overlay(shape.strokeBorder(palette.color(.line), lineWidth: metrics.borderWidth))
        }
        .buttonStyle(.plain)
    }
}

#Preview("Text only") {
    ZStack {
        ChipView(item: ChipModel(id: .daita, name: "Example")) {}
    }
    .frame(maxWidth: .infinity, maxHeight: .infinity)
    .background(UIColor.secondaryColor.color)
}

#Preview("Text + icon") {
    ZStack {
        ChipView(item: ChipModel(id: .daita, name: "Example", icon: .warrenIconMultihopWhenNeeded)) {}
    }
    .frame(maxWidth: .infinity, maxHeight: .infinity)
    .background(UIColor.secondaryColor.color)
}
