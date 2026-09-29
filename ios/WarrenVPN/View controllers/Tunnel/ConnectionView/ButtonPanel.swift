//
//  ButtonPanel.swift
//  MullvadVPN
//
//  Created by Andrew Bulhak on 2025-01-03.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import SwiftUI

extension ConnectionView {
    internal struct ButtonPanel: View {
        typealias Action = (ConnectionViewViewModel.TunnelAction) -> Void

        @ObservedObject var viewModel: ConnectionViewViewModel
        var action: Action?

        var body: some View {
            VStack(spacing: ConnectSurfaceMetrics.Card.blockGap) {
                locationButtons(with: action)
                    .disabled(viewModel.disableButtons)
                actionButton(with: action)
                    .disabled(viewModel.disableButtons)
            }
        }

        /// One control for the location, with the shuffle on its side, in every
        /// tunnel state: two separate rounded buttons on one row (desktop
        /// `SelectLocationButtons`). The reconnect half the other clients no
        /// longer offer stays gone. Android's twin is `SwitchLocationButton.kt`.
        @ViewBuilder
        private func locationButtons(with action: Action?) -> some View {
            HStack(spacing: ConnectSurfaceMetrics.Button.rowGap) {
                Button(action: { action?(.selectLocation) }) {
                    Text(viewModel.localizedTitleForSelectLocationButton)
                }
                .buttonStyle(CardButtonStyle(tone: .neutral))
                .accessibilityIdentifier(AccessibilityIdentifier.selectLocationButton.asString)

                Button(action: { action?(.shuffleLocation) }) {
                    Image(systemName: "shuffle")
                        .font(.system(size: 14, weight: .semibold))
                }
                .buttonStyle(CardButtonStyle(tone: .neutral))
                .frame(width: ConnectSurfaceMetrics.Button.shuffleWidth)
                .disabled(!viewModel.shuffleEnabled)
                .accessibilityLabel(LocalizedStringKey("Random location"))
                .accessibilityHint(LocalizedStringKey("Connect to a randomly selected location"))
                .accessibilityIdentifier(AccessibilityIdentifier.shuffleLocationButton.asString)
            }
        }

        @ViewBuilder
        private func actionButton(with action: Action?) -> some View {
            let fill = CardButtonStyle(tone: .action(viewModel.actionButtonFillToken))
            switch viewModel.actionButton {
            case .connect:
                Button(action: { action?(.connect) }) {
                    Text(LocalizedStringKey("Connect"))
                }
                .buttonStyle(fill)
                .accessibilityIdentifier(AccessibilityIdentifier.connectButton.asString)
            case .disconnect:
                Button(action: { action?(.disconnect) }) {
                    Text(LocalizedStringKey("Disconnect"))
                }
                .buttonStyle(fill)
                .accessibilityIdentifier(AccessibilityIdentifier.disconnectButton.asString)
            case .cancel:
                let disconnects = viewModel.tunnelStatus.state == .waitingForConnectivity(.noConnection)
                Button(action: { action?(.cancel) }) {
                    Text(LocalizedStringKey(disconnects ? "Disconnect" : "Cancel"))
                }
                .buttonStyle(fill)
                .accessibilityIdentifier(
                    disconnects
                        ? AccessibilityIdentifier.disconnectButton.asString
                        : AccessibilityIdentifier.cancelButton.asString
                )
            }
        }
    }
}

#Preview {
    ConnectionViewComponentPreview(showIndicators: true) { _, vm, _ in
        ConnectionView.ButtonPanel(viewModel: vm, action: nil)
    }
}
