//
//  WarrenWalletIdentityView.swift
//  WarrenVPN
//
//  Created by Warren on 2026-05-22 (C.6 follow-up).
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  Settings → Wallet → "Show identity". Read-only view that surfaces
//  the user's Warren SS58 wallet address (`wb…`) for support ticket
//  attachment. Does NOT reveal the seed or mnemonic ; the address is
//  non-secret and only identifies the wallet on the Warren exit
//  allowlist.
//

import SwiftUI

public struct WarrenWalletIdentityView: View {
    /// The Warren SS58 wallet address (`wb…`, 47-49 chars). Populated
    /// from `WarrenWallet.publicKeyAddress` by the caller. The full
    /// address is displayed (selectable) and copied ; the short form is
    /// shown as a compact headline.
    public let address: String

    @State private var didCopy: Bool = false

    public init(address: String) {
        self.address = address
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text(
                String(
                    localized: "Wallet identity",
                    table: "Wallet",
                    comment: "Title of the read-only pubkey display view"
                )
            )
            .font(.warrenBig)
            .foregroundColor(.white)

            Text(
                String(
                    localized: "Your wallet address is shown below. It is safe to share with Warren support to identify your account ; it cannot be used to access your wallet or sign on your behalf.",
                    table: "Wallet",
                    comment: "Explanatory body for the wallet address display view"
                )
            )
            .font(.warrenMicro)
            .foregroundColor(.white.opacity(0.7))

            // Compact headline (first 6 + … + last 6) for quick visual
            // identification ; the full address remains below.
            Text(address.shortWarrenAddress)
                .font(.warrenSmallSemiBold)
                .foregroundColor(.white.opacity(0.7))

            // Full address, selectable so the user can copy any portion.
            // The Copy button below always copies the full address.
            Text(address)
                .font(.system(.body, design: .monospaced))
                .foregroundColor(.Warren.yellow)
                .padding(12)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(
                    RoundedRectangle(cornerRadius: 8)
                        .fill(Color.Warren.surface)
                )
                .textSelection(.enabled)
                .accessibilityLabel(
                    String(
                        localized: "Wallet address",
                        table: "Wallet",
                        comment: "VoiceOver label for the wallet SS58 address text"
                    )
                )

            Button(action: copyPubkey) {
                HStack {
                    Image(systemName: didCopy ? "checkmark" : "doc.on.doc")
                    Text(
                        didCopy
                        ? String(
                            localized: "Copied",
                            table: "Wallet",
                            comment: "Transient feedback after pubkey copy"
                        )
                        : String(
                            localized: "Copy to clipboard",
                            table: "Wallet",
                            comment: "Pubkey copy button"
                        )
                    )
                }
                .font(.warrenSmallSemiBold)
                .foregroundColor(.Warren.navy)
                .padding(.horizontal, 16)
                .padding(.vertical, 10)
                .background(
                    RoundedRectangle(cornerRadius: 8)
                        .fill(Color.Warren.yellow)
                )
            }

            Spacer()
        }
        .padding(16)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Color.Warren.navy)
    }

    private func copyPubkey() {
        // Always copy the FULL address, never the abbreviated headline.
        UIPasteboard.general.string = address
        didCopy = true
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) {
            didCopy = false
        }
    }
}
