//
//  WarrenMnemonicDisplayView.swift
//  WarrenVPN
//
//  Created by Warren on 2026-05-21.
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  SwiftUI 12-word BIP39 mnemonic display, used during onboarding
//  (generate new wallet, backup gate) and in Settings backup (Face ID
//  gated). The grid / copy / acknowledge sub-views are shared with the
//  onboarding wizard's backup reminder step.
//

import SwiftUI
import UIKit

/// 12-word BIP39 mnemonic backup view. The phrase is shown directly with a
/// copy button (aligned with the desktop app). Copying clears the clipboard
/// after 60 seconds, but only if it still holds this phrase.
///
/// With `requiresAcknowledgement` (the wallet-creation backup gate) the
/// confirm button stays disabled until the user explicitly checks the
/// "I have written it down" row, matching the desktop login backup step.
///
/// Accessibility: VoiceOver reads each word as `index. word` (e.g. "1. wisdom").
public struct WarrenMnemonicDisplayView: View {
    /// The 12-word mnemonic to display (space-separated).
    public let mnemonic: String

    /// Gate the confirm button behind the explicit acknowledgement row.
    public let requiresAcknowledgement: Bool

    /// Callback when the user confirms they have written the words down.
    public var onConfirmed: () -> Void

    @State private var acknowledged = false

    public init(mnemonic: String, requiresAcknowledgement: Bool = false, onConfirmed: @escaping () -> Void) {
        self.mnemonic = mnemonic
        self.requiresAcknowledgement = requiresAcknowledgement
        self.onConfirmed = onConfirmed
    }

    public var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 20) {
                Text(String(localized: "Your 12 recovery words", table: "Wallet"))
                    .font(.warrenLarge)
                    .foregroundColor(.white)
                WarrenMnemonicWarningCallout()
                WarrenMnemonicGrid(mnemonic: mnemonic)
                WarrenMnemonicCopyButton(mnemonic: mnemonic)
                if requiresAcknowledgement {
                    WarrenAcknowledgeRow(
                        isOn: $acknowledged,
                        label: String(
                            localized: "I have written down my recovery phrase in a safe place.",
                            table: "Wallet"
                        ),
                        identifier: .walletBackupAcknowledgeToggle
                    )
                }
                confirmButton
            }
            .padding()
        }
        .background(Color.Warren.navy)
        // No container identifier: it would propagate to and clobber the
        // copy/confirm button ids below. UI tests detect this screen via
        // the confirm button.
    }

    private var isConfirmEnabled: Bool {
        !requiresAcknowledgement || acknowledged
    }

    private var confirmButton: some View {
        Button(action: onConfirmed) {
            Text(
                requiresAcknowledgement
                    ? String(localized: "Continue", table: "Wallet")
                    : String(localized: "I have written them down", table: "Wallet")
            )
            .font(.warrenSmallSemiBold)
            .frame(maxWidth: .infinity)
            .padding(.vertical, 14)
            .background(Color.Warren.success.opacity(isConfirmEnabled ? 1 : 0.4))
            .foregroundColor(.white.opacity(isConfirmEnabled ? 1 : 0.6))
            .cornerRadius(10)
        }
        .disabled(!isConfirmEnabled)
        .accessibilityIdentifier(AccessibilityIdentifier.walletMnemonicConfirmButton.asString)
    }
}

/// Red-tinted warning box above the phrase, matching the desktop
/// `DangerCallout` on the login backup step.
public struct WarrenMnemonicWarningCallout: View {
    public init() {}

    public var body: some View {
        Text(String(
            localized: "Write them down on paper and store them somewhere safe. Anyone with access to these words can recover your Warren wallet.",
            table: "Wallet"
        ))
        .font(.warrenMini)
        .foregroundColor(.white)
        .fixedSize(horizontal: false, vertical: true)
        .padding(.vertical, 8)
        .padding(.horizontal, 12)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(
            RoundedRectangle(cornerRadius: 8)
                .fill(Color.Warren.error.opacity(0.25))
        )
        .overlay(
            RoundedRectangle(cornerRadius: 8)
                .stroke(Color.Warren.error.opacity(0.5), lineWidth: 1)
        )
    }
}

/// The numbered 3-column word grid.
public struct WarrenMnemonicGrid: View {
    public let mnemonic: String

    public init(mnemonic: String) {
        self.mnemonic = mnemonic
    }

    private var words: [String] {
        mnemonic.split(separator: " ").map(String.init)
    }

    public var body: some View {
        LazyVGrid(columns: [GridItem(.flexible()), GridItem(.flexible()), GridItem(.flexible())], spacing: 12) {
            ForEach(Array(words.enumerated()), id: \.offset) { idx, word in
                wordCell(index: idx, word: word)
            }
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

/// Copy button with the transient "Copied" confirmation. Copying clears
/// the clipboard after 60 seconds, but only if it still holds the phrase,
/// so it never wipes something the user copied afterwards.
public struct WarrenMnemonicCopyButton: View {
    public let mnemonic: String

    @State private var didCopy = false

    public init(mnemonic: String) {
        self.mnemonic = mnemonic
    }

    public var body: some View {
        Button(action: copyMnemonic) {
            Label(
                didCopy
                    ? String(localized: "Copied", table: "Wallet")
                    : String(localized: "Copy", table: "Wallet"),
                systemImage: didCopy ? "checkmark" : "doc.on.doc"
            )
            .font(.warrenSmall)
            .foregroundColor(.Warren.yellow)
        }
        .accessibilityAddTraits(.isButton)
        .accessibilityIdentifier(AccessibilityIdentifier.walletMnemonicCopyButton.asString)
    }

    private func copyMnemonic() {
        let phrase = mnemonic
        UIPasteboard.general.string = phrase
        didCopy = true
        DispatchQueue.main.asyncAfter(deadline: .now() + 60) {
            if UIPasteboard.general.string == phrase {
                UIPasteboard.general.string = ""
            }
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + 2) {
            didCopy = false
        }
    }
}

/// Checkbox-style acknowledgement row used by the backup gates.
public struct WarrenAcknowledgeRow: View {
    @Binding public var isOn: Bool
    public let label: String
    public let identifier: AccessibilityIdentifier

    public init(isOn: Binding<Bool>, label: String, identifier: AccessibilityIdentifier) {
        self._isOn = isOn
        self.label = label
        self.identifier = identifier
    }

    public var body: some View {
        Button(action: { isOn.toggle() }) {
            HStack(alignment: .top, spacing: 12) {
                Image(systemName: isOn ? "checkmark.square.fill" : "square")
                    .font(.title3)
                    .foregroundColor(isOn ? .Warren.success : .white.opacity(0.6))
                Text(label)
                    .font(.warrenTiny)
                    .foregroundColor(.white)
                    .fixedSize(horizontal: false, vertical: true)
                    .multilineTextAlignment(.leading)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .accessibilityAddTraits(.isButton)
        .accessibilityValue(isOn ? "1" : "0")
        .accessibilityIdentifier(identifier.asString)
    }
}
