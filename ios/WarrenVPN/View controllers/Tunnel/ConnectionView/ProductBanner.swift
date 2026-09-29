//
//  ProductBanner.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import SwiftUI

/// The marker of a non-prod build over the connect screen's scenery, and the
/// way in to what its network costs in speed. Desktop's `BetaBadge` overlay.
///
/// An opaque surface of the theme, like the card: a translucent one took the
/// landscape's hue. One line whatever the language: the banner widens to its
/// text rather than wrapping it.
struct ProductBanner: View {
    let badge: String
    let line: String
    let action: () -> Void

    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let palette = ConnectSurfacePalette(colorScheme: colorScheme)
        let metrics = ConnectSurfaceMetrics.Banner.self
        let shape = RoundedRectangle(cornerRadius: metrics.radius)

        Button(action: action) {
            HStack(spacing: metrics.gap) {
                Text(verbatim: badge)
                    .connectFont(size: metrics.pillTextSize, weight: .bold, relativeTo: .caption)
                    .kerning(metrics.pillLetterSpacing)
                    .foregroundStyle(palette.color(.pillText))
                    .padding(.horizontal, metrics.pillPaddingHorizontal)
                    .frame(minHeight: metrics.pillHeight)
                    .background(
                        RoundedRectangle(cornerRadius: metrics.pillRadius)
                            .fill(palette.color(.pill))
                    )
                Text(verbatim: line)
                    .connectFont(size: metrics.textSize, weight: .semibold, relativeTo: .caption)
                    .foregroundStyle(palette.color(.text))
                    .lineLimit(1)
                    // A language longer than the screen shrinks the line a
                    // little before it would ever cut it.
                    .minimumScaleFactor(0.75)
            }
            .padding(.leading, metrics.paddingStart)
            .padding(.trailing, metrics.paddingEnd)
            .frame(minHeight: metrics.height)
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
            .contentShape(shape)
        }
        .buttonStyle(.plain)
        .accessibilityLabel(
            String(
                format: NSLocalizedString("%@ build, about this network", tableName: "Settings", comment: ""),
                badge
            )
        )
        .accessibilityHint(line)
    }
}
