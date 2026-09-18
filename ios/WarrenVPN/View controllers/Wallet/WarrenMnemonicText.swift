//
//  WarrenMnemonicText.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  Counting and canonicalizing a typed BIP39 phrase. Held apart from the view
//  so both can be exercised without SwiftUI; the Android twins are
//  `countMnemonicWords` and `normalizeMnemonic` in `MnemonicInput.kt`, and the
//  desktop's are in `MnemonicTextarea.tsx`.
//

import Foundation

enum WarrenMnemonicText {
    /// The two lengths BIP39 defines, and the only two the daemon accepts.
    static let validWordCounts = [12, 24]

    /// Whitespace-separated words in `input`.
    static func wordCount(_ input: String) -> Int {
        input.split(whereSeparator: \.isWhitespace).count
    }

    static func isValidWordCount(_ input: String) -> Bool {
        validWordCounts.contains(wordCount(input))
    }

    /// The length the counter counts up to: 12 until the user goes past it,
    /// then 24. Nothing else would let one field serve both phrases.
    static func target(for input: String) -> Int {
        wordCount(input) > 12 ? 24 : 12
    }

    /// Canonicalizes a pasted phrase before it goes to the daemon: trims,
    /// lowercases, and collapses runs of whitespace to single spaces.
    ///
    /// Never call this on the way IN to the field. Trimming while the user
    /// types swallows the space they just pressed between two words, which is
    /// the bug the Android twin carries the same warning about.
    static func normalize(_ input: String) -> String {
        input.split(whereSeparator: \.isWhitespace)
            .map { $0.lowercased() }
            .joined(separator: " ")
    }
}
