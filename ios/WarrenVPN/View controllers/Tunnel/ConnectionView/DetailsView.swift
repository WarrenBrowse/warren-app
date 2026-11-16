//
//  DetailsView.swift
//  MullvadVPN
//
//  Created by Andrew Bulhak on 2025-01-03.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import SwiftUI

/// The connection details the home opens over its hero: the transport, the
/// hops, and where the traffic goes in and comes out.
struct ConnectionDetailsView: View {
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
                    .font(.instrumentSans(.footnote, weight: .semibold))
                    .foregroundStyle(palette.color(.homeTextSecondary))
                Spacer()
            }

            if let tunnelProtocolName = viewModel.tunnelProtocolName {
                Text(verbatim: tunnelProtocolName)
                    .font(.instrumentSans(.subheadline))
                    .foregroundStyle(palette.color(.homeText))
            }
            if viewModel.isMultihop {
                Text(LocalizedStringKey("Multihop (2 hops)"))
                    .font(.instrumentSans(.subheadline))
                    .foregroundStyle(palette.color(.homeText))
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
                .font(.instrumentSans(.subheadline))
                .foregroundStyle(palette.color(.homeTextSecondary))
                .frame(minWidth: columnWidth, alignment: .leading)
                .sizeOfView { columnWidth = max(columnWidth, $0.width) }
                .accessibilityHidden(true)
            Text(value)
                .font(.instrumentSans(.subheadline))
                .foregroundStyle(palette.color(.homeText))
                .accessibilityLabel(Text(title) + Text(verbatim: " \(value)"))
                .accessibilityIdentifier(accessibilityId.asString)
        }
    }
}
