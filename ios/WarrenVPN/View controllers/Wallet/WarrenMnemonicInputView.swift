//
//  WarrenMnemonicInputView.swift
//  WarrenVPN
//
//  Created by Warren on 2026-05-21.
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  BIP39 recovery-phrase entry, used during wallet restore and onboarding.
//

import SwiftUI

/// One paste-friendly field for the whole phrase, with a live word counter.
///
/// This was a grid of twelve cells. A 24-word wallet could not be imported at
/// all: pasting into the first cell split the phrase and kept its first twelve
/// words, and the CTA only appeared when all twelve were non-empty, so the
/// screen both truncated the phrase and then told the user it was ready. The
/// Android and desktop clients had already moved to a single field for the same
/// reason, and this is their twin (`MnemonicInput.kt`, `MnemonicTextarea.tsx`).
///
/// The field is controlled: `phrase` is the single source of truth and is
/// handed back verbatim, so what the CTA is enabled on is always what is on
/// screen. The phrase is canonicalized on the way OUT, never on the way in;
/// trimming while the user types swallows the space between two words.
///
/// The daemon does the real wordlist and checksum validation; the counter is
/// feedback only.
public struct WarrenMnemonicInputView: View {
    /// Fired with the canonicalized phrase once it is a valid length.
    public var onComplete: (String) -> Void

    @State private var phrase: String = ""

    public init(onComplete: @escaping (String) -> Void) {
        self.onComplete = onComplete
    }

    private var wordCount: Int { WarrenMnemonicText.wordCount(phrase) }
    private var isValidLength: Bool { WarrenMnemonicText.isValidWordCount(phrase) }

    public var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text(
                String(
                    localized: "Enter your 12-word (or 24-word) recovery phrase, separated by spaces.",
                    table: "Wallet"
                )
            )
            .font(.warrenLarge)
            .foregroundColor(.white)
            .fixedSize(horizontal: false, vertical: true)

            TextEditor(text: $phrase)
                .font(.system(.body, design: .monospaced))
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled(true)
                .scrollContentBackground(.hidden)
                .frame(minHeight: 96)
                .padding(8)
                .background(
                    RoundedRectangle(cornerRadius: 6)
                        .stroke(borderColor, lineWidth: 1)
                )
                .foregroundColor(.white)
                .accessibilityIdentifier(
                    AccessibilityIdentifier.walletMnemonicPhraseField.asString)
                .accessibilityLabel(Text(String(localized: "Recovery phrase", table: "Wallet")))
                // Keeps the typed phrase out of screenshots the system takes.
                .privacySensitive()

            Text("\(wordCount) / \(WarrenMnemonicText.target(for: phrase)) words")
                .font(.warrenTinySemiBold)
                .foregroundColor(.white.opacity(0.6))
                .accessibilityIdentifier("walletMnemonicWordCount")

            // Always on screen, disabled until the phrase is a valid length: a
            // button that appears out of nowhere gives the user nothing to aim
            // at while they type.
            Button(action: submit) {
                Text(String(localized: "Restore account", table: "Wallet"))
                    .font(.warrenSmallSemiBold)
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 14)
                    .background(isValidLength ? Color.Warren.yellow : Color.gray)
                    .foregroundColor(.black)
                    .cornerRadius(10)
            }
            .disabled(!isValidLength)
            .accessibilityIdentifier(
                AccessibilityIdentifier.walletMnemonicRestoreSubmitButton.asString)
        }
        .padding()
        .background(Color.Warren.navy)
    }

    private var borderColor: Color {
        if phrase.isEmpty { return .gray.opacity(0.5) }
        return isValidLength ? Color.Warren.yellow.opacity(0.6) : .gray.opacity(0.5)
    }

    private func submit() {
        guard isValidLength else { return }
        onComplete(WarrenMnemonicText.normalize(phrase))
    }
}

// Warren brand colors live in `UIColor+Warren.swift` for cross-target
// (UIKit + SwiftUI) consumption.
