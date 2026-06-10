//
//  WarrenMnemonicDisplayView.swift
//  WarrenVPN
//
//  Created by Warren on 2026-05-21.
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  SwiftUI 12-word BIP39 mnemonic display, used during onboarding
//  (generate new wallet) and in Settings backup (Face ID gated).
//

import SwiftUI
import UIKit

/// 12-word BIP39 mnemonic backup view. The phrase is shown directly with a
/// copy button (aligned with the desktop app). Copying clears the clipboard
/// after 60 seconds, but only if it still holds this phrase.
///
/// Accessibility: VoiceOver reads each word as `index. word` (e.g. "1. wisdom").
public struct WarrenMnemonicDisplayView: View {
    /// The 12-word mnemonic to display (space-separated).
    public let mnemonic: String

    /// Callback when the user confirms they have written the words down.
    public var onConfirmed: () -> Void

    @State private var didCopy: Bool = false

    public init(mnemonic: String, onConfirmed: @escaping () -> Void) {
        self.mnemonic = mnemonic
        self.onConfirmed = onConfirmed
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 24) {
            VStack(alignment: .leading, spacing: 8) {
                Text(String(localized: "Your 12 recovery words", table: "Wallet"))
                    .font(.mullvadLarge)
                    .foregroundColor(.white)
                Text(String(localized: "Write them down on paper and store them somewhere safe. Anyone with access to these words can recover your Warren wallet.", table: "Wallet"))
                    .font(.mullvadTiny)
                    .foregroundColor(.white.opacity(0.7))
            }
            LazyVGrid(columns: [GridItem(.flexible()), GridItem(.flexible()), GridItem(.flexible())], spacing: 12) {
                ForEach(Array(words.enumerated()), id: \.offset) { idx, word in
                    wordCell(index: idx, word: word)
                }
            }
            Button(action: copyMnemonic) {
                Label(
                    didCopy
                        ? String(localized: "Copied", table: "Wallet")
                        : String(localized: "Copy", table: "Wallet"),
                    systemImage: didCopy ? "checkmark" : "doc.on.doc"
                )
                .font(.mullvadSmall)
                .foregroundColor(.Warren.yellow)
            }
            .accessibilityAddTraits(.isButton)
            Button(action: onConfirmed) {
                Text(String(localized: "I have written them down", table: "Wallet"))
                    .font(.mullvadSmallSemiBold)
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 14)
                    .background(Color.Warren.yellow)
                    .foregroundColor(.black)
                    .cornerRadius(10)
            }
        }
        .padding()
        .background(Color.Warren.navy)
    }

    private var words: [String] {
        mnemonic.split(separator: " ").map(String.init)
    }

    private func copyMnemonic() {
        let phrase = mnemonic
        UIPasteboard.general.string = phrase
        didCopy = true
        // Clear the clipboard after a minute, but only if it still holds the
        // phrase, so we never wipe something the user copied afterwards.
        DispatchQueue.main.asyncAfter(deadline: .now() + 60) {
            if UIPasteboard.general.string == phrase {
                UIPasteboard.general.string = ""
            }
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + 2) {
            didCopy = false
        }
    }

    @ViewBuilder
    private func wordCell(index: Int, word: String) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text("\(index + 1)")
                .font(.caption2)
                .foregroundColor(.white.opacity(0.6))
            Text(word)
                .font(.body.monospaced())
                .foregroundColor(.white)
                .padding(.vertical, 8)
                .padding(.horizontal, 6)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(
                    RoundedRectangle(cornerRadius: 6)
                        .stroke(Color.Warren.yellow.opacity(0.4), lineWidth: 1)
                )
                .accessibilityLabel("\(index + 1). \(word)")
        }
    }
}
