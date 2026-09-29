//
//  HeaderView.swift
//  MullvadVPN
//
//  Created by Andrew Bulhak on 2025-01-03.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import SwiftUI

extension ConnectionView {
    internal struct HeaderView: View {
        @ObservedObject var viewModel: ConnectionViewViewModel
        @Binding var isExpanded: Bool

        @Environment(\.colorScheme) private var colorScheme

        var body: some View {
            let palette = ConnectSurfacePalette(colorScheme: colorScheme)
            let metrics = ConnectSurfaceMetrics.Status.self
            let phase = viewModel.connectionPhase

            HStack(alignment: .center, spacing: metrics.rowGap) {
                // The eye sits in a quiet well of the phase hue (desktop
                // ConnectionStatus): crossed out = hidden, open = visible.
                RoundedRectangle(cornerRadius: metrics.wellRadius)
                    .fill(palette.color(phase.cardWellToken))
                    .frame(width: metrics.wellSize, height: metrics.wellSize)
                    .overlay {
                        Image(systemName: viewModel.eyeSymbolName)
                            .font(.system(size: metrics.iconSize - 2, weight: .medium))
                            .frame(width: metrics.iconSize, height: metrics.iconSize)
                            .foregroundStyle(palette.color(phase.cardTitleToken))
                    }
                    .animation(.easeInOut(duration: 0.3), value: phase)
                    .accessibilityIdentifier("connectionStatusEye")
                    .accessibilityHidden(true)

                VStack(alignment: .leading, spacing: metrics.textGap) {
                    Text(viewModel.localizedTitleForSecureLabel)
                        .connectFont(
                            size: metrics.titleSize,
                            weight: .semibold,
                            lineHeight: metrics.titleLineHeight,
                            relativeTo: .headline
                        )
                        .foregroundStyle(palette.color(phase.cardTitleToken))
                        .accessibilityIdentifier(viewModel.accessibilityIdForSecureLabel.asString)
                        .accessibilityLabel(viewModel.localizedAccessibilityLabelForSecureLabel)
                        .accessibilityRemoveTraits(.isButton)

                    if let subtitle = viewModel.localizedSubtitleForSecureLabel {
                        Text(subtitle)
                            .connectFont(
                                size: metrics.subtitleSize,
                                lineHeight: metrics.subtitleLineHeight,
                                relativeTo: .footnote
                            )
                            .foregroundStyle(palette.color(.textSecondary))
                            .accessibilityIdentifier("connectionStatusSubtitle")
                    }
                }

                Spacer(minLength: 0)

                HStack(spacing: metrics.trailingGap) {
                    Image(.iconChevronUp)
                        .renderingMode(.template)
                        .resizable()
                        .scaledToFit()
                        .frame(
                            width: ConnectSurfaceMetrics.Card.chevronIconSize,
                            height: ConnectSurfaceMetrics.Card.chevronIconSize
                        )
                        .rotationEffect(isExpanded ? .degrees(-180) : .degrees(0))
                        .frame(
                            width: ConnectSurfaceMetrics.Card.chevronBoxSize,
                            height: ConnectSurfaceMetrics.Card.chevronBoxSize
                        )
                        .foregroundStyle(palette.color(.text))
                        .accessibilityRemoveTraits(.isImage)
                        .accessibilityLabel(
                            isExpanded
                                ? LocalizedStringKey("Collapse connection details")
                                : LocalizedStringKey("Expand connection details")
                        )
                        .showIf(viewModel.showsConnectionDetails)

                    // The flag owns the end of the row in EVERY state so it
                    // never appears to move; the chevron slots in on its left.
                    if let flag = viewModel.currentCountryFlagEmoji {
                        CountryFlag(emoji: flag, ring: palette.color(.line))
                    }
                }
            }
            .accessibilityElement(children: .contain)
            .contentShape(Rectangle())
            .onTapGesture {
                guard viewModel.showsConnectionDetails else { return }
                withAnimation {
                    isExpanded.toggle()
                }
            }
            .accessibilityIdentifier(
                AccessibilityIdentifier.relayStatusCollapseButton.asString
            )
        }
    }
}

/// The flag as a round badge with a hairline ring (desktop CurrentCountryFlag).
/// The emoji is drawn larger than the circle so its rectangle fills the disc.
private struct CountryFlag: View {
    let emoji: String
    let ring: Color

    var body: some View {
        let metrics = ConnectSurfaceMetrics.Flag.self
        Text(verbatim: emoji)
            .font(.system(size: metrics.size * 1.45))
            .frame(width: metrics.size, height: metrics.size)
            .clipShape(Circle())
            .overlay(Circle().strokeBorder(ring, lineWidth: metrics.ringWidth))
            .accessibilityHidden(true)
    }
}

#Preview {
    ConnectionViewComponentPreview(showIndicators: true) { _, vm, isExpanded in
        ConnectionView.HeaderView(viewModel: vm, isExpanded: isExpanded)
    }
}
