//
//  WarrenSecureMnemonicTests.swift
//  WarrenVPNTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import WarrenRustRuntime
import XCTest

@testable import WarrenVPN

/// The recovery phrase in a buffer that can actually be erased.
///
/// A Swift `String` cannot be: it is immutable, copies on assignment, keeps
/// short values inline, and bridges to `NSString` behind this code's back. The
/// point of this type is that exactly one copy exists and a wipe reaches it.
final class WarrenSecureMnemonicTests: XCTestCase {
    /// The BIP39 test vector, so the derivation tests exercise a phrase the
    /// wordlist and the checksum both accept.
    private let phrase = "abandon abandon abandon abandon abandon abandon "
        + "abandon abandon abandon abandon abandon about"

    func testThePhraseCrossesToRustUnchanged() {
        let secure = WarrenSecureMnemonic(phrase: phrase)

        let read = secure.withCString { String(cString: $0) }

        XCTAssertEqual(read, phrase)
    }

    /// Reading from `Data` is how the Keychain hands it over, and that path
    /// must never build a `String` on the way in.
    func testAPhraseReadFromTheKeychainIsTheSamePhrase() {
        let secure = WarrenSecureMnemonic(data: Data(phrase.utf8))

        XCTAssertEqual(secure.withCString { String(cString: $0) }, phrase)
        XCTAssertEqual(secure.wordCount, 12)
    }

    /// The whole reason this type exists: after a wipe there is nothing left
    /// to read, not a shorter string and not a stale one.
    func testAWipedPhraseIsGoneRatherThanShortened() {
        let secure = WarrenSecureMnemonic(phrase: phrase)
        XCTAssertFalse(secure.isEmpty)

        secure.wipe()

        XCTAssertTrue(secure.isEmpty)
        XCTAssertEqual(secure.wordCount, 0)
        XCTAssertEqual(secure.withCString { String(cString: $0) }, "")
        XCTAssertEqual(secure.revealForDisplay(), "")
    }

    /// Wiping twice must not be a second free or a crash: every caller wipes
    /// on its own path out, and deinit wipes again.
    func testWipingIsIdempotent() {
        let secure = WarrenSecureMnemonic(phrase: phrase)
        secure.wipe()
        secure.wipe()
        XCTAssertTrue(secure.isEmpty)
    }

    /// The count is what the import screen checks, so it must survive the
    /// spacing a paste carries and never need the words themselves.
    func testTheWordCountSurvivesWhateverSpacingAPasteCarries() {
        XCTAssertEqual(WarrenSecureMnemonic(phrase: "  one   two\ttwelve \n ").wordCount, 3)
        XCTAssertEqual(WarrenSecureMnemonic(phrase: "").wordCount, 0)
        XCTAssertEqual(WarrenSecureMnemonic(phrase: "   ").wordCount, 0)
        XCTAssertEqual(WarrenSecureMnemonic(phrase: phrase).wordCount, 12)
    }

    /// An empty phrase must not allocate zero bytes and then write to them.
    func testAnEmptyPhraseIsHandledWithoutTouchingMemoryItDoesNotOwn() {
        let secure = WarrenSecureMnemonic(phrase: "")
        XCTAssertTrue(secure.isEmpty)
        XCTAssertEqual(secure.withCString { String(cString: $0) }, "")
    }

    /// The phrase goes Keychain to Rust without becoming a `String`, and the
    /// wallet it derives holds none: a wallet used only to sign must not carry
    /// a secret it cannot erase.
    func testAWalletDerivedFromASecurePhraseCarriesNoPhraseOfItsOwn() throws {
        let secure = WarrenSecureMnemonic(phrase: phrase)

        let wallet = try WarrenWallet.fromMnemonic(secure)

        XCTAssertNil(wallet.mnemonic)
        XCTAssertEqual(wallet.seed.count, 32)
        XCTAssertEqual(wallet.publicKey.count, 32)
    }

    /// The two entries must derive the same identity: the secure one is only
    /// a different way of carrying the same bytes.
    func testBothEntriesDeriveTheSameIdentity() throws {
        let fromString = try WarrenWallet.fromMnemonic(phrase)
        let fromSecure = try WarrenWallet.fromMnemonic(WarrenSecureMnemonic(phrase: phrase))

        XCTAssertEqual(fromString.publicKey, fromSecure.publicKey)
        XCTAssertEqual(fromString.seed, fromSecure.seed)
    }

    /// A phrase the wordlist refuses must fail the same way through either
    /// entry, rather than yielding a wallet with a zero seed.
    func testAPhraseTheWordlistRefusesFailsThroughEitherEntry() {
        let nonsense = WarrenSecureMnemonic(phrase: "not a real bip39 phrase at all")
        XCTAssertThrowsError(try WarrenWallet.fromMnemonic(nonsense))
        XCTAssertThrowsError(try WarrenWallet.fromMnemonic("not a real bip39 phrase at all"))
    }

    /// Forgetting zeroes the seed for real, which is the part that can be
    /// zeroed; the phrase is dropped because a `String` cannot be.
    func testForgettingZeroesTheSeedRatherThanOnlyDroppingIt() throws {
        let wallet = try WarrenWallet.fromMnemonic(phrase)
        XCTAssertNotNil(wallet.mnemonic)
        XCTAssertTrue(wallet.seed.contains { $0 != 0 })

        wallet.forgetSecret()

        XCTAssertNil(wallet.mnemonic)
        XCTAssertTrue(wallet.seed.allSatisfy { $0 == 0 })
    }
}
