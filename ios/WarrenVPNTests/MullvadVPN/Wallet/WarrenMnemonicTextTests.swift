//
//  WarrenMnemonicTextTests.swift
//  WarrenVPNTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import XCTest

@testable import WarrenVPN

/// A 24-word recovery phrase could not be imported on iOS at all: the restore
/// screen was a hard-coded grid of 12 cells that truncated a pasted 24-word
/// phrase to its first 12 and then asked the daemon to accept it. Anyone whose
/// wallet was created with 24 words was locked out of this client.
final class WarrenMnemonicTextTests: XCTestCase {
    private let twelve = "abandon abandon abandon abandon abandon abandon "
        + "abandon abandon abandon abandon abandon about"
    private var twentyFour: String { twelve + " " + twelve }

    func testBothBip39LengthsCount() {
        XCTAssertEqual(WarrenMnemonicText.wordCount(twelve), 12)
        XCTAssertEqual(WarrenMnemonicText.wordCount(twentyFour), 24)
    }

    func testBothBip39LengthsAreAccepted() {
        XCTAssertTrue(WarrenMnemonicText.isValidWordCount(twelve))
        XCTAssertTrue(WarrenMnemonicText.isValidWordCount(twentyFour))
    }

    func testAnyOtherLengthIsRefused() {
        XCTAssertFalse(WarrenMnemonicText.isValidWordCount(""))
        XCTAssertFalse(WarrenMnemonicText.isValidWordCount("abandon about"))
        XCTAssertFalse(WarrenMnemonicText.isValidWordCount(twelve + " extra"))
        XCTAssertFalse(WarrenMnemonicText.isValidWordCount(twentyFour + " extra"))
    }

    /// A phrase pasted from a password manager arrives with newlines, tabs,
    /// double spaces and capitals, and every one of those is still the phrase.
    func testAPastedPhraseIsCountedWhateverTheWhitespaceAroundIt() {
        let messy = "  Abandon\tabandon\nabandon   abandon abandon abandon "
            + "abandon abandon abandon abandon abandon ABOUT \n"
        XCTAssertEqual(WarrenMnemonicText.wordCount(messy), 12)
        XCTAssertEqual(WarrenMnemonicText.normalize(messy), twelve)
    }

    func testNormalizingLowercasesAndCollapsesToSingleSpaces() {
        XCTAssertEqual(WarrenMnemonicText.normalize("  ONE   two\tTHREE\n"), "one two three")
        XCTAssertEqual(WarrenMnemonicText.normalize(""), "")
    }

    /// The counter has to change its target the moment the phrase passes 12,
    /// or a 24-word phrase reads "24 / 12 words" and looks wrong while correct.
    func testTheCounterTargetFollowsThePhrasePastTwelve() {
        XCTAssertEqual(WarrenMnemonicText.target(for: ""), 12)
        XCTAssertEqual(WarrenMnemonicText.target(for: twelve), 12)
        XCTAssertEqual(WarrenMnemonicText.target(for: twelve + " thirteen"), 24)
        XCTAssertEqual(WarrenMnemonicText.target(for: twentyFour), 24)
    }
}
