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

        var body: some View {
            HStack(alignment: .center, spacing: 12) {
                // Phase-colored status eye (desktop Bula card iconography):
                // crossed-out = hidden/protected, open = visible/exposed.
                Image(systemName: viewModel.eyeSymbolName)
                    .font(.title3.weight(.semibold))
                    .foregroundStyle(viewModel.textColorForSecureLabel.color)
                    .accessibilityIdentifier("connectionStatusEye")
                    .accessibilityHidden(true)

                VStack(alignment: .leading, spacing: 0) {
                    Text(viewModel.localizedTitleForSecureLabel)
                        .font(.title3.weight(.semibold))
                        .foregroundStyle(viewModel.textColorForSecureLabel.color)
                        .accessibilityIdentifier(viewModel.accessibilityIdForSecureLabel.asString)
                        .accessibilityLabel(viewModel.localizedAccessibilityLabelForSecureLabel)
                        .accessibilityRemoveTraits(.isButton)

                    if let subtitle = viewModel.localizedSubtitleForSecureLabel {
                        Text(subtitle)
                            .font(.footnote)
                            .foregroundStyle(UIColor.primaryTextColor.color.opacity(0.6))
                            .accessibilityIdentifier("connectionStatusSubtitle")
                    }
                }

                Spacer()

                Image(.iconChevronUp)
                    .renderingMode(.template)
                    .rotationEffect(isExpanded ? .degrees(-180) : .degrees(0))
                    .foregroundStyle(.white)
                    .accessibilityRemoveTraits(.isImage)
                    .accessibilityLabel(
                        isExpanded
                            ? LocalizedStringKey("Collapse connection details")
                            : LocalizedStringKey("Expand connection details")
                    )
                    .showIf(viewModel.showsConnectionDetails)

                // The flag owns the top-right corner in EVERY state so it
                // never appears to move; the chevron slots in on its left.
                if let flag = viewModel.currentCountryFlagEmoji {
                    Text(flag)
                        .font(.system(size: 22))
                        .accessibilityHidden(true)
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

#Preview {
    ConnectionViewComponentPreview(showIndicators: true) { _, vm, isExpanded in
        ConnectionView.HeaderView(viewModel: vm, isExpanded: isExpanded)
    }
}
