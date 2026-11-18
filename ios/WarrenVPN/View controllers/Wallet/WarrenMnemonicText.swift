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

    /// The counter under the phrase field. Worded as "Words: 3 / 12" so no
    /// language has to inflect the noun for the count.
    static func counter(for input: String, bundle: Bundle = .main) -> String {
        let format = NSLocalizedString(
            "Words: %lld / %lld",
            tableName: "Wallet",
            bundle: bundle,
            comment: "Words typed so far, out of the phrase length."
        )
        return String(format: format, wordCount(input), target(for: input))
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

    /// How many words of a freshly shown recovery phrase the user types back
    /// before the account is used. Three out of twelve is what a person who
    /// wrote the phrase down answers in seconds and someone who only ticked a
    /// box cannot. The desktop and Android twins are `backup-check.ts` and
    /// `BackupCheck.kt`.
    static let backupCheckWordCount = 3

    /// Draws the zero-based positions of the words to ask for, distinct and in
    /// phrase order.
    static func backupCheckPositions(
        wordCount: Int,
        random: () -> Double = { Double.random(in: 0 ..< 1) }
    ) -> [Int] {
        let wanted = min(backupCheckWordCount, wordCount)
        var positions = Set<Int>()
        while positions.count < wanted {
            positions.insert(min(wordCount - 1, Int(random() * Double(wordCount))))
        }
        return positions.sorted()
    }

    static func backupCheckPasses(mnemonic: String, positions: [Int], answers: [String]) -> Bool {
        let words = normalize(mnemonic).split(separator: " ").map(String.init)
        guard !positions.isEmpty, positions.count == answers.count else { return false }
        return zip(positions, answers).allSatisfy { position, answer in
            let given = answer.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
            return !given.isEmpty && words.indices.contains(position) && given == words[position]
        }
    }
}
