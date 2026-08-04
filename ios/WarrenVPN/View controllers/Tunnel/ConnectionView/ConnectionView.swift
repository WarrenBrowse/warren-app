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

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Spacer()
                .accessibilityIdentifier(AccessibilityIdentifier.connectionView.asString)
                .accessibilityHidden(true)

            // Active features float ABOVE the glass card as a stack of pills
            // over the scenery (desktop StyledFeatureBadges), not inside it.
            ChipContainerView(
                viewModel: indicatorsViewModel,
                tunnelState: connectionViewModel.tunnelStatus.state,
                isExpanded: .constant(true)
            )
            .padding(.horizontal, 16)
            .showIf(hasFeatureIndicators && connectionViewModel.showsConnectionDetails)

            VStack(spacing: 16) {
                VStack(alignment: .leading, spacing: 0) {
                    HeaderView(viewModel: connectionViewModel, isExpanded: $isExpanded)
                        .padding(.bottom, 4)

                    Divider()
                        .background(UIColor.secondaryTextColor.color)
                        .padding(.top, 4)
                        .padding(.bottom, 8)
                        .accessibilityHidden(true)
                        .showIf(isExpanded)

                    ScrollView {
                        VStack(alignment: .leading, spacing: 2) {
                            if let titleForCountryAndCity = connectionViewModel.titleForCountryAndCity {
                                Text(titleForCountryAndCity)
                                    .lineLimit(isExpanded ? 2 : 1)
                                    .font(.title3.weight(.semibold))
                                    .foregroundStyle(UIColor.primaryTextColor.color)
                                    .accessibilityHidden(true)
                            }
                            if let titleForServer = connectionViewModel.titleForServer {
                                Text(titleForServer)
                                    .lineLimit(isExpanded ? 3 : 1)
                                    .font(.body)
                                    .foregroundStyle(UIColor.primaryTextColor.color.opacity(0.6))
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

                                    // Warren-specific: always-on HTTP/3
                                    // mimicry indicator. Shown inside expanded
                                    // details only when the tunnel is secured
                                    // - surfaces the baseline obfuscation so
                                    // users know their traffic is
                                    // indistinguishable from regular HTTPS.
                                    WarrenObfuscationIndicatorView()
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
                }
                .transformEffect(.identity)
                .animation(.default, value: hasFeatureIndicators)
                ButtonPanel(viewModel: connectionViewModel, action: action)
            }
            .padding(16)
            // Glass card over the scenery (desktop connection card): denser
            // black when expanded, material blur underneath so the artwork
            // shimmers through, hairline border and soft drop shadow.
            .background {
                RoundedRectangle(cornerRadius: 16)
                    .fill(.ultraThinMaterial)
                    .overlay(
                        RoundedRectangle(cornerRadius: 16)
                            .fill(Color.black.opacity(isExpanded ? 0.6 : 0.5))
                    )
            }
            .environment(\.colorScheme, .dark)
            .clipShape(RoundedRectangle(cornerRadius: 16))
            .overlay(RoundedRectangle(cornerRadius: 16).strokeBorder(Color.white.opacity(0.2), lineWidth: 1))
            .shadow(color: Color.black.opacity(0.35), radius: 12, x: 0, y: 8)
            .padding(EdgeInsets(top: 0, leading: 16, bottom: 8, trailing: 16))
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
