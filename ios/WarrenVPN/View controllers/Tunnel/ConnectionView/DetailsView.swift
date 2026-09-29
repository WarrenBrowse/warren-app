//
//  DetailsView.swift
//  MullvadVPN
//
//  Created by Andrew Bulhak on 2025-01-03.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import SwiftUI

extension ConnectionView {
    internal struct DetailsView: View {
        @ObservedObject var viewModel: ConnectionViewViewModel
        @State private var columnWidth: CGFloat = 0
        @Environment(\.colorScheme) private var colorScheme

        private var palette: ConnectSurfacePalette {
            ConnectSurfacePalette(colorScheme: colorScheme)
        }

        var body: some View {
            VStack(alignment: .leading, spacing: 8) {
                HStack {
                    Text(LocalizedStringKey("Connection details"))
                        .font(.footnote.weight(.semibold))
                        .foregroundStyle(palette.color(.textMuted))
                    Spacer()
                }

                if let tunnelProtocolName = viewModel.tunnelProtocolName {
                    Text(verbatim: tunnelProtocolName)
                        .font(.subheadline)
                        .foregroundStyle(palette.color(.text))
                }
                if viewModel.isMultihop {
                    Text(LocalizedStringKey("Multihop (2 hops)"))
                        .font(.subheadline)
                        .foregroundStyle(palette.color(.text))
                }

                VStack(alignment: .leading, spacing: 0) {
                    if let inAddress = viewModel.inAddress {
                        connectionDetailRow(
                            title: LocalizedStringKey("In"),
                            value: inAddress,
                            accessibilityId: .connectionPanelInAddressRow
                        )
                    }
                    // No egress conncheck on the Warren network: the "Out" row
                    // falls back to the exit location the daemon provides.
                    if viewModel.tunnelIsConnected, let outExitLocation = viewModel.outExitLocation {
                        connectionDetailRow(
                            title: LocalizedStringKey("Out"),
                            value: outExitLocation,
                            accessibilityId: .connectionPanelOutAddressRow
                        )
                    }
                }
            }
            .animation(.default, value: viewModel.inAddress)
            .animation(.default, value: viewModel.tunnelIsConnected)
        }

        @ViewBuilder
        private func connectionDetailRow(
            title: LocalizedStringKey,
            value: String,
            accessibilityId: AccessibilityIdentifier
        ) -> some View {
            HStack(alignment: .top, spacing: 8) {
                Text(title)
                    .font(.subheadline)
                    .foregroundStyle(palette.color(.textMuted))
                    .frame(minWidth: columnWidth, alignment: .leading)
                    .sizeOfView { columnWidth = max(columnWidth, $0.width) }
                    .accessibilityHidden(true)
                Text(value)
                    .font(.subheadline)
                    .foregroundStyle(palette.color(.text))
                    .accessibilityLabel(Text(title) + Text(verbatim: " \(value)"))
                    .accessibilityIdentifier(accessibilityId.asString)
            }
        }
    }
}

#Preview {
    ConnectionViewComponentPreview(showIndicators: true) { _, vm, _ in
        ConnectionView.DetailsView(viewModel: vm)
    }
}
