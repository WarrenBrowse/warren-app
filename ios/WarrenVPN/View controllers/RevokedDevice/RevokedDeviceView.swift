//
//  RevokedDeviceView.swift
//  MullvadVPN
//
//  Created by Jon Petersson on 2025-10-02.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import SwiftUI

struct RevokedDeviceView: View {
    @StateObject var viewModel: RevokedDeviceViewModel
    var onLogout: (() -> Void)?

    /// Revocation is the registry's only way to end a device (warren-core
    /// doc 115), and it always comes from another device of the account.
    static var bodyText: String {
        NSLocalizedString(
            "This device was disconnected from another device of your account. Sign in with your recovery phrase to use it again.",
            comment: ""
        )
    }

    var body: some View {
        VStack(spacing: 0) {
            ScrollView {
                VStack(alignment: .leading, spacing: 0) {
                    HStack {
                        Spacer()
                        Image.warrenIconFail
                        Spacer()
                    }

                    Text("Device is inactive")
                        .font(.warrenLarge)
                        .foregroundStyle(Color.MullvadText.onBackgroundEmphasis100)
                        .padding(.top, 16)

                    Text(verbatim: Self.bodyText)
                        .font(.warrenSmall)
                        .foregroundStyle(Color.MullvadText.onBackground)
                        .padding(.top, 8)

                    Text("Going to login will unblock the Internet on this device.")
                        .font(.warrenSmall)
                        .foregroundStyle(Color.MullvadText.onBackground)
                        .padding(.top, 16)
                        .showIf(viewModel.tunnelState.isSecured)
                }
                .padding(.top, 24)
            }
            .scrollBounceBehavior(.automatic)
            MainButton(text: "Go to login", style: viewModel.tunnelState.isSecured ? .danger : .default) {
                onLogout?()
            }
            .accessibilityIdentifier(.revokedDeviceLoginButton)
        }
        .padding(.horizontal, 16)
        .padding(.bottom, 16)
        .background(Color.warrenBackground)
    }
}

#Preview("Secured") {
    RevokedDeviceView(
        viewModel: RevokedDeviceViewModel(
            interactor: MockRevokedDeviceInteractor(
                tunnelStatus: TunnelStatus(
                    observedState: .error(.init(reason: .deviceRevoked)),
                    state: .error(.deviceRevoked)
                )
            )
        )
    )
}

#Preview("Not secured") {
    RevokedDeviceView(
        viewModel: RevokedDeviceViewModel(
            interactor: MockRevokedDeviceInteractor(
                tunnelStatus: TunnelStatus(
                    observedState: .disconnected,
                    state: .disconnected
                )
            )
        )
    )
}
