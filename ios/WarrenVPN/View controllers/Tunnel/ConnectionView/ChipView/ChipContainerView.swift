//
//  ChipContainerView.swift
//  MullvadVPN
//
//  Created by Mojgan on 2024-12-05.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import SwiftUI

/// The active features, as the vertical stack of pills the art direction asks
/// for: left-aligned, one per row, every one of them visible.
///
/// This was a wrapping horizontal flow with an "N more..." escape hatch, which
/// is neither what desktop draws (`FeatureIndicators.tsx`, a flex column at
/// `gap: 5px`) nor what Android draws (`FeatureIndicatorsPanel.kt`, a `Column`
/// at `Dimens.chipStackGap`). The flow also measured its own rows by hand
/// through alignment guides and reported its height back through a state
/// variable, so it re-laid out twice per appearance; a `VStack` sizes itself.
struct ChipContainerView<ViewModel>: View where ViewModel: ChipViewModelProtocol {
    @ObservedObject var viewModel: ViewModel

    var body: some View {
        VStack(alignment: .leading, spacing: UIMetrics.FeatureIndicators.chipStackGap) {
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
