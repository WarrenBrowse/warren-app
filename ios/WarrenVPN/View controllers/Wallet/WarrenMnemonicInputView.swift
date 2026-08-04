//
//  WarrenMnemonicInputView.swift
//  WarrenVPN
//
//  Created by Warren on 2026-05-21.
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  SwiftUI 12-word BIP39 input view, used during wallet restore and
//  onboarding (import existing wallet).
//

import SwiftUI

/// 12-word BIP39 mnemonic entry. Paste the full phrase (space-separated)
/// into any word field to auto-fill all 12 fields; otherwise type each
/// word individually with BIP39-wordlist client-side validation.
///
/// Validates on-the-fly via `isValid(word:)` callback that the consumer
/// wires to the BIP39 wordlist (a future FFI export from
/// `warren-identity` will provide this; scaffold version returns true
/// for any non-empty trimmed word).
public struct WarrenMnemonicInputView: View {
    /// Callback fired when all 12 words are filled and pass validation.
    public var onComplete: (String) -> Void

    /// Predicate for individual word validation. Default returns true
    /// for any non-empty trimmed lowercased word. Production wires to
    /// the BIP39 wordlist from `warren-identity` via FFI.
    public var isValid: (String) -> Bool = { word in
        !word.trimmingCharacters(in: .whitespaces).isEmpty
    }

    @State private var words: [String] = Array(repeating: "", count: 12)
    @State private var focusedIndex: Int? = 0

    /// 4 rows of 3 columns for portrait iPhone; 6 rows of 2 for
    /// compact-width contexts. SwiftUI Grid adapts automatically when
    /// wrapped in `GeometryReader` (production task).
    public var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text(String(localized: "Enter your 12 recovery words", table: "Wallet"))
                .font(.warrenLarge)
                .foregroundColor(.white)
            LazyVGrid(columns: [GridItem(.flexible()), GridItem(.flexible()), GridItem(.flexible())], spacing: 12) {
                ForEach(0..<12, id: \.self) { idx in
                    wordCell(index: idx)
                }
            }
            if isComplete {
                Button(action: submitIfValid) {
                    Text(String(localized: "Restore account", table: "Wallet"))
                        .font(.warrenSmallSemiBold)
                        .frame(maxWidth: .infinity)
                        .padding(.vertical, 14)
                        .background(allWordsValid ? Color.Warren.yellow : Color.gray)
                        .foregroundColor(.black)
                        .cornerRadius(10)
                }
                .disabled(!allWordsValid)
                .accessibilityIdentifier(AccessibilityIdentifier.walletMnemonicRestoreSubmitButton.asString)
            }
        }
        .padding()
        .background(Color.Warren.navy)
        // No container identifier: it would propagate to and clobber the
        // per-word field ids and the restore button id below. UI tests
        // detect this screen via the first word field.
    }

    @ViewBuilder
    private func wordCell(index: Int) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text("\(index + 1)")
                .font(.caption2)
                .foregroundColor(.white.opacity(0.6))
            TextField("", text: Binding(
                get: { words[index] },
                set: { newValue in
                    let lowered = newValue.lowercased()
                    if lowered.contains(" ") {
                        // Paste-full-phrase support: split by spaces, fill all cells.
                        let split = lowered.split(separator: " ").map(String.init)
                        for (i, w) in split.prefix(12).enumerated() {
                            words[i] = w
                        }
                    } else {
                        words[index] = lowered
                    }
                }
            ))
            .textInputAutocapitalization(.never)
            .autocorrectionDisabled(true)
            // Per-cell identifier so UI tests can target a specific word
            // field (e.g. paste the full phrase into the first cell).
            .accessibilityIdentifier("walletMnemonicWordField_\(index)")
            .padding(8)
            .background(
                RoundedRectangle(cornerRadius: 6)
                    .stroke(borderColor(for: index), lineWidth: 1)
            )
            .foregroundColor(.white)
        }
    }

    private func borderColor(for index: Int) -> Color {
        let word = words[index].trimmingCharacters(in: .whitespaces)
        if word.isEmpty { return .gray.opacity(0.5) }
        return isValid(word) ? Color.Warren.yellow.opacity(0.6) : Color.red.opacity(0.8)
    }

    private var isComplete: Bool {
        words.allSatisfy { !$0.trimmingCharacters(in: .whitespaces).isEmpty }
    }

    private var allWordsValid: Bool {
        words.allSatisfy { isValid($0.trimmingCharacters(in: .whitespaces)) }
    }

    private func submitIfValid() {
        guard allWordsValid else { return }
        let mnemonic = words
            .map { $0.trimmingCharacters(in: .whitespaces) }
            .joined(separator: " ")
        onComplete(mnemonic)
    }
}

// Warren brand colors live in `UIColor+Warren.swift` for cross-target
// (UIKit + SwiftUI) consumption.
