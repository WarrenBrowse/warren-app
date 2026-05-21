//
//  WarrenMnemonicDisplayView.swift
//  WarrenVPN
//
//  Created by Warren on 2026-05-21.
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  Scaffold for C.5 — UI Swift wallet Ed25519 mnemonic auth.
//  SwiftUI 12-word BIP39 mnemonic display with blur+reveal pattern
//  (no copy button by design: reduces clipboard-malware attack surface).
//  Used during onboarding wizard Step 2a (Generate new wallet) and in
//  Settings → Backup → View mnemonic (Face ID gated). NOT yet wired
//  into the Xcode project.
//

import SwiftUI

/// 12-word BIP39 mnemonic backup view. The phrase is hidden under a
/// blur overlay by default; tap-and-hold reveals it, releasing hides
/// it again. No copy button: the user is expected to hand-write the
/// 12 words during backup.
///
/// Accessibility: VoiceOver reads each word as `index. word` (e.g.
/// "1. wisdom") only when the reveal gesture is active.
public struct WarrenMnemonicDisplayView: View {
    /// The 12-word mnemonic to display (space-separated).
    public let mnemonic: String

    /// Callback when the user confirms they have written the words down.
    public var onConfirmed: () -> Void

    @State private var isRevealed: Bool = false

    public init(mnemonic: String, onConfirmed: @escaping () -> Void) {
        self.mnemonic = mnemonic
        self.onConfirmed = onConfirmed
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 24) {
            VStack(alignment: .leading, spacing: 8) {
                Text("Your 12 recovery words")
                    .font(.headline)
                    .foregroundColor(.white)
                Text("Write them down on paper and store them somewhere safe. Anyone with access to these words can recover your Warren wallet.")
                    .font(.caption)
                    .foregroundColor(.white.opacity(0.7))
            }
            LazyVGrid(columns: [GridItem(.flexible()), GridItem(.flexible()), GridItem(.flexible())], spacing: 12) {
                ForEach(Array(words.enumerated()), id: \.offset) { idx, word in
                    wordCell(index: idx, word: word)
                }
            }
            .overlay(blurOverlay)
            .gesture(
                LongPressGesture(minimumDuration: 0.2)
                    .onChanged { _ in isRevealed = true }
                    .onEnded { _ in
                        // Re-hide after a short delay.
                        DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) {
                            isRevealed = false
                        }
                    }
            )
            Text("Press and hold to reveal. Do not screenshot.")
                .font(.caption2)
                .foregroundColor(.white.opacity(0.5))
            Button(action: onConfirmed) {
                Text("I have written them down")
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 12)
                    .background(Color.warrenYellow)
                    .foregroundColor(.black)
                    .cornerRadius(8)
            }
        }
        .padding()
        .background(Color.warrenNavy)
    }

    private var words: [String] {
        mnemonic.split(separator: " ").map(String.init)
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
                        .stroke(Color.warrenYellow.opacity(0.4), lineWidth: 1)
                )
                .accessibilityLabel("\(index + 1). \(isRevealed ? word : "hidden")")
        }
    }

    @ViewBuilder
    private var blurOverlay: some View {
        if !isRevealed {
            RoundedRectangle(cornerRadius: 6)
                .fill(Color.warrenNavy.opacity(0.85))
                .overlay(
                    Image(systemName: "eye.slash.fill")
                        .foregroundColor(.warrenYellow)
                        .font(.title)
                )
                .allowsHitTesting(false)
        }
    }
}
