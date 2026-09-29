//
//  ConnectionView.swift
//  MullvadVPN
//
//  Created by Jon Petersson on 2024-12-03.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import SwiftUI

struct ConnectionView: View {
    @ObservedObject var connectionViewModel: ConnectionViewViewModel
    @ObservedObject var indicatorsViewModel: FeatureIndicatorsViewModel

    @State private(set) var isExpanded = false

    @State private(set) var scrollViewHeight: CGFloat = 0
    var hasFeatureIndicators: Bool { !indicatorsViewModel.chips.isEmpty }
    var action: ButtonPanel.Action?
    /// The card's top edge in window coordinates, reported on every frame of
    /// its height animation. The scenery backdrop places the scene against it,
    /// so Bula keeps his footing above the card instead of being swallowed
    /// when the connection details expand.
    var cardTopChanged: ((CGFloat) -> Void)?

    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let palette = ConnectSurfacePalette(colorScheme: colorScheme)
        let card = ConnectSurfaceMetrics.Card.self
        let cardShape = RoundedRectangle(cornerRadius: card.radius)

        VStack(alignment: .leading, spacing: card.chipsToCardGap) {
            if let badge = connectionViewModel.productBadge {
                ProductBanner(badge: badge, line: connectionViewModel.productBannerLine) {
                    action?(.explainNetwork)
                }
                .padding(.top, 13.5)
                .padding(.leading, 15)
                .padding(.trailing, card.marginHorizontal)
            }

            Spacer()
                .accessibilityIdentifier(AccessibilityIdentifier.connectionView.asString)
                .accessibilityHidden(true)

            // Active features float ABOVE the card as a stack of pills over
            // the scenery (desktop StyledFeatureBadges), not inside it.
            ChipContainerView(viewModel: indicatorsViewModel)
                .padding(.horizontal, card.marginHorizontal)
                .showIf(hasFeatureIndicators && connectionViewModel.showsConnectionDetails)

            VStack(alignment: .leading, spacing: 0) {
                HeaderView(viewModel: connectionViewModel, isExpanded: $isExpanded)
                    .padding(.bottom, isExpanded ? 16 : 0)
                    .overlay(alignment: .bottom) {
                        Rectangle()
                            .fill(palette.color(.line))
                            .frame(height: 1)
                            .showIf(isExpanded)
                            .accessibilityHidden(true)
                    }

                ScrollView {
                    VStack(alignment: .leading, spacing: 0) {
                        if let titleForCountryAndCity = connectionViewModel.titleForCountryAndCity {
                            Text(titleForCountryAndCity)
                                .lineLimit(isExpanded ? 2 : 1)
                                .connectFont(
                                    size: ConnectSurfaceMetrics.Location.size,
                                    weight: .bold,
                                    lineHeight: ConnectSurfaceMetrics.Location.lineHeight,
                                    relativeTo: .headline
                                )
                                .foregroundStyle(palette.color(.text))
                                .padding(.top, ConnectSurfaceMetrics.Location.gapAbove)
                                .accessibilityHidden(true)
                        }
                        if let titleForServer = connectionViewModel.titleForServer {
                            Text(titleForServer)
                                .lineLimit(isExpanded ? 3 : 1)
                                .connectFont(
                                    size: ConnectSurfaceMetrics.Location.hostnameSize,
                                    lineHeight: ConnectSurfaceMetrics.Location.hostnameLineHeight,
                                    relativeTo: .footnote
                                )
                                .foregroundStyle(palette.color(.textMuted))
                                .padding(.top, ConnectSurfaceMetrics.Location.hostnameGapAbove)
                                .accessibilityIdentifier(
                                    AccessibilityIdentifier.connectionPanelServerLabel.asString
                                )
                                .accessibilityLabel(connectionViewModel.accessibilityLabelForServer ?? "")
                                .multilineTextAlignment(.leading)
                                .fixedSize(horizontal: false, vertical: true)
                        }
                        HStack {
                            VStack(alignment: .leading, spacing: 0) {
                                DetailsView(viewModel: connectionViewModel)
                                    .padding(.vertical, 8)
                                    .showIf(isExpanded)

                                // Warren-specific: always-on HTTP/3 mimicry
                                // indicator, shown in the expanded details
                                // only while the tunnel is secured, so users
                                // know their traffic looks like regular HTTPS.
                                WarrenObfuscationIndicatorView(palette: palette)
                                    .padding(.top, 8)
                                    .padding(.bottom, 4)
                                    .showIf(isExpanded && connectionViewModel.tunnelStatus.state.isSecured)
                            }
                            Spacer()
                        }
                    }.frame(maxWidth: .infinity, alignment: .leading)
                        .sizeOfView { size in
                            withAnimation {
                                scrollViewHeight = size.height
                            }
                        }
                }
                .frame(maxHeight: scrollViewHeight)
                .scrollBounceBehavior(.basedOnSize)
                .transformEffect(.identity)
                .animation(.default, value: hasFeatureIndicators)

                ButtonPanel(viewModel: connectionViewModel, action: action)
                    .padding(.top, card.blockGap)
            }
            .padding(card.padding)
            // Opaque, in the theme's paper: a translucent card let each
            // landscape tint it, so it read olive on one country and slate on
            // the next.
            .background {
                cardShape
                    .fill(palette.color(.card))
                    .shadow(
                        color: palette.color(.shadowStrong),
                        radius: card.shadowBlur / 2,
                        x: 0,
                        y: card.shadowOffsetY
                    )
            }
            .overlay(cardShape.strokeBorder(palette.color(.line), lineWidth: card.borderWidth))
            .padding(
                EdgeInsets(
                    top: 0,
                    leading: card.marginHorizontal,
                    bottom: card.marginBottom,
                    trailing: card.marginHorizontal
                )
            )
            .topOfView { top in
                cardTopChanged?(top)
            }
            .onChange(of: connectionViewModel.showsConnectionDetails) {
                if !connectionViewModel.showsConnectionDetails {
                    withAnimation {
                        isExpanded = false
                    }
                }
            }
        }
    }
}

#Preview("ConnectionView (Indicators)") {
    ConnectionViewComponentPreview(showIndicators: true) { indicatorModel, viewModel, _ in
        ConnectionView(connectionViewModel: viewModel, indicatorsViewModel: indicatorModel)
    }
}

#Preview("ConnectionView (No indicators)") {
    ConnectionViewComponentPreview(showIndicators: false) { indicatorModel, viewModel, _ in
        ConnectionView(connectionViewModel: viewModel, indicatorsViewModel: indicatorModel)
    }
}
