//
//  ChipContainerView.swift
//  MullvadVPN
//
//  Created by Mojgan on 2024-12-05.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import SwiftUI

/// The active features, as the vertical stack of pills the art direction asks
/// for: left-aligned, one per row, every one of them visible, 2 apart
/// (`connectionCard.badgeGap` in design-tokens.json, desktop
/// `StyledFeatureBadges`, Android `FeatureIndicatorsPanel.kt`).
struct ChipContainerView<ViewModel>: View where ViewModel: ChipViewModelProtocol {
    @ObservedObject var viewModel: ViewModel

    var body: some View {
        VStack(alignment: .leading, spacing: ConnectSurfaceMetrics.Card.chipGap) {
            ForEach(viewModel.chips) { data in
                ChipView(item: data) {
                    viewModel.onPressed(item: data)
                }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

#Preview("Feature stack") {
    ChipContainerView(viewModel: MockFeatureIndicatorsViewModel())
        .padding(.horizontal, 16)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .bottomLeading)
        .background(UIColor.secondaryColor.color)
}
