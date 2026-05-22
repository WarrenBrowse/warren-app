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
import UIKit

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

    /// `true` while the user-screenshot warning alert is presented.
    /// Triggered by `UIApplication.userDidTakeScreenshotNotification`
    /// when iOS reports a screenshot was just captured — Warren cannot
    /// scrub the screenshot from the Photos library (iOS doesn't
    /// expose that API), but we can immediately re-hide the mnemonic
    /// AND surface a warning that the screenshot leaked.
    @State private var screenshotWarningPresented: Bool = false

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
            Text(String(localized: "Press and hold to reveal. Do not screenshot.", table: "Wallet"))
                .font(.mullvadMicro)
                .foregroundColor(.white.opacity(0.5))
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
        .onReceive(NotificationCenter.default.publisher(for: UIApplication.userDidTakeScreenshotNotification)) { _ in
            // Force-hide the mnemonic + surface a strong-language
            // warning. The screenshot has already been captured —
            // iOS doesn't expose a way to scrub it from Photos —
            // but immediate re-hide prevents follow-on screenshots
            // showing a different angle, and the alert tells the user
            // the leak happened so they can rotate the wallet.
            isRevealed = false
            screenshotWarningPresented = true
        }
        .alert(
            String(
                localized: "Screenshot detected",
                table: "Wallet",
                comment: "Title of the alert shown when iOS reports a screenshot was just taken"
            ),
            isPresented: $screenshotWarningPresented
        ) {
            Button(
                String(localized: "Close", table: "Wallet", comment: ""),
                role: .cancel
            ) {}
        } message: {
            Text(
                String(
                    localized: "Your recovery phrase has been captured in a screenshot. Anyone with access to that screenshot can recover your wallet. Generate a new wallet and move your funds if you cannot guarantee the screenshot stays private.",
                    table: "Wallet",
                    comment: "Body of the post-screenshot warning explaining the threat model"
                )
            )
        }
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
                        .stroke(Color.Warren.yellow.opacity(0.4), lineWidth: 1)
                )
                .accessibilityLabel("\(index + 1). \(isRevealed ? word : "hidden")")
        }
    }

    @ViewBuilder
    private var blurOverlay: some View {
        if !isRevealed {
            RoundedRectangle(cornerRadius: 6)
                .fill(Color.Warren.navy.opacity(0.85))
                .overlay(
                    Image(systemName: "eye.slash.fill")
                        .foregroundColor(Color.Warren.yellow)
                        .font(.title)
                )
                .allowsHitTesting(false)
        }
    }
}
