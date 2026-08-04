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
            VStack(spacing: 16) {
                HStack(spacing: 8) {
                    locationButton(with: action)
                        .disabled(viewModel.disableButtons)
                    shuffleButton(with: action)
                        .disabled(viewModel.disableButtons)
                }
                actionButton(with: action)
                    .disabled(viewModel.disableButtons)
            }
        }

        /// Desktop-parity "surprise me" button: connects to a randomly
        /// picked exit instead of opening the location picker.
        @ViewBuilder
        private func shuffleButton(with action: Action?) -> some View {
            Button(
                action: { action?(.shuffleLocation) },
                label: {
                    Image(systemName: "shuffle")
                        .resizable()
                        .scaledToFit()
                        .padding(12)
                        .frame(width: 44, height: 44)
                }
            )
            .buttonStyle(MainButtonStyle(.default))
            .cornerRadius(UIMetrics.MainButton.cornerRadius)
            .accessibilityLabel(LocalizedStringKey("Random location"))
            .accessibilityHint(LocalizedStringKey("Connect to a randomly selected location"))
            .accessibilityIdentifier(AccessibilityIdentifier.shuffleLocationButton.asString)
        }

        @ViewBuilder
        private func locationButton(with action: Action?) -> some View {
            switch viewModel.tunnelStatus.state {
            case .connecting, .connected, .reconnecting, .waitingForConnectivity, .negotiatingEphemeralPeer, .error:
                SplitMainButton(
                    text: viewModel.localizedTitleForSelectLocationButton,
                    image: .iconReload,
                    style: .default,
                    accessibilityId: .selectLocationButton,
                    secondaryAccessibilityId: .reconnectButton,
                    secondaryAccessibilityLabel: LocalizedStringKey("Reconnect"),
                    secondaryAccessibilityHint: LocalizedStringKey("Cycle through available servers"),
                    primaryAction: { action?(.selectLocation) },
                    secondaryAction: { action?(.reconnect) }
                )
            case .disconnecting, .pendingReconnect, .disconnected:
                MainButton(
                    text: viewModel.localizedTitleForSelectLocationButton,
                    style: .default,
                    action: { action?(.selectLocation) }
                )
                .accessibilityIdentifier(AccessibilityIdentifier.selectLocationButton.asString)
            }
        }

        @ViewBuilder
        private func actionButton(with action: Action?) -> some View {
            switch viewModel.actionButton {
            case .connect:
                MainButton(
                    text: LocalizedStringKey("Connect"),
                    style: .success,
                    action: { action?(.connect) }
                )
                .accessibilityIdentifier(AccessibilityIdentifier.connectButton.asString)
            case .disconnect:
                MainButton(
                    text: LocalizedStringKey("Disconnect"),
                    style: .danger,
                    action: { action?(.disconnect) }
                )
                .accessibilityIdentifier(AccessibilityIdentifier.disconnectButton.asString)
            case .cancel:
                MainButton(
                    text: LocalizedStringKey(
                        viewModel.tunnelStatus.state == .waitingForConnectivity(.noConnection)
                            ? "Disconnect"
                            : "Cancel"
                    ),
                    style: .danger,
                    action: { action?(.cancel) }
                )
                .accessibilityIdentifier(
                    viewModel.tunnelStatus.state == .waitingForConnectivity(.noConnection)
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
