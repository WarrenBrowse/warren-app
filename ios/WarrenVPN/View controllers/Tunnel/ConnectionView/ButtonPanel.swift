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
                locationButton(with: action)
                    .disabled(viewModel.disableButtons)
                actionButton(with: action)
                    .disabled(viewModel.disableButtons)
            }
        }

        /// One control for the location, with the shuffle on its side, in every
        /// tunnel state. It used to be three: a split button whose side half
        /// was a reconnect the other clients no longer offer, plus a detached
        /// shuffle square next to it, plus the plain label in the disconnected
        /// states. Sharing the row with that square is what squeezed the label
        /// enough to wrap "Changer de localisation" onto two lines.
        /// Android's twin is `SwitchLocationButton.kt`.
        @ViewBuilder
        private func locationButton(with action: Action?) -> some View {
            SplitMainButton(
                text: viewModel.localizedTitleForSelectLocationButton,
                systemImage: "shuffle",
                style: .default,
                accessibilityId: .selectLocationButton,
                secondaryAccessibilityId: .shuffleLocationButton,
                secondaryAccessibilityLabel: LocalizedStringKey("Random location"),
                secondaryAccessibilityHint: LocalizedStringKey("Connect to a randomly selected location"),
                secondaryEnabled: viewModel.shuffleEnabled,
                primaryAction: { action?(.selectLocation) },
                secondaryAction: { action?(.shuffleLocation) }
            )
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
