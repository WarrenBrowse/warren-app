//
//  WarrenWalletIdentityView.swift
//  WarrenVPN
//
//  Created by Warren on 2026-05-22 (C.6 follow-up).
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  Settings → Wallet → "Show identity". Read-only view that surfaces
//  the user's Warren Ed25519 public key (hex-encoded) for support
//  ticket attachment. Does NOT reveal the seed or mnemonic ; the
//  pubkey is non-secret and only identifies the wallet on the Warren
//  exit allowlist.
//

import SwiftUI

public struct WarrenWalletIdentityView: View {
    /// 32-byte Ed25519 pubkey rendered as a 64-char lowercase hex
    /// string. Populated from `WarrenWallet.publicKey` by the caller.
    public let pubkeyHex: String

    @State private var didCopy: Bool = false

    public init(pubkeyHex: String) {
        self.pubkeyHex = pubkeyHex
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
            .font(.mullvadBig)
            .foregroundColor(.white)

            Text(
                String(
                    localized: "Your wallet's public key is shown below. It is safe to share with Warren support to identify your account ; it cannot be used to access your wallet or sign on your behalf.",
                    table: "Wallet",
                    comment: "Explanatory body for the pubkey display view"
                )
            )
            .font(.mullvadMicro)
            .foregroundColor(.white.opacity(0.7))

            Text(pubkeyHex)
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
                        localized: "Wallet public key, hex encoded",
                        table: "Wallet",
                        comment: "VoiceOver label for the pubkey hex text"
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
                .font(.mullvadSmallSemiBold)
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
        UIPasteboard.general.string = pubkeyHex
        didCopy = true
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) {
            didCopy = false
        }
    }
}
