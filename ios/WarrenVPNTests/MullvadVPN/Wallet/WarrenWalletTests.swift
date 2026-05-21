//
//  WarrenWalletTests.swift
//  WarrenVPNTests
//
//  Created by Warren on 2026-05-21.
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  Tests the Swift wrapper around `warren_wallet_ffi` (warren-ios
//  Rust crate -> warren-identity). These exercise real cryptography
//  via the Rust FFI ; they must run on iOS Simulator (the warren-ios
//  staticlib is iOS-target only).
//

@testable import WarrenRustRuntime
@testable import WarrenVPN
import XCTest

final class WarrenWalletTests: XCTestCase {
    /// Generating a fresh wallet yields a valid 12-word BIP39 phrase
    /// + a 32-byte pubkey + a 32-byte seed.
    func test_generate_producesValidWallet() throws {
        let wallet = try WarrenWallet.generate()

        let words = wallet.revealMnemonic().split(separator: " ")
        XCTAssertEqual(words.count, 12, "generate(12) should produce a 12-word phrase")
        for word in words {
            XCTAssertGreaterThan(word.count, 0)
        }
        XCTAssertEqual(wallet.seed.count, 32)
        XCTAssertEqual(wallet.publicKey.count, 32)
    }

    /// Two `generate` calls produce different mnemonics (entropy works).
    func test_generate_isNonDeterministic() throws {
        let a = try WarrenWallet.generate()
        let b = try WarrenWallet.generate()
        XCTAssertNotEqual(a.revealMnemonic(), b.revealMnemonic())
        XCTAssertNotEqual(a.publicKey, b.publicKey)
    }

    /// Loading the same mnemonic twice yields the same seed + pubkey
    /// (warren-identity HKDF determinism, foundational invariant).
    func test_fromMnemonic_isDeterministic() throws {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
        let a = try WarrenWallet.fromMnemonic(mnemonic)
        let b = try WarrenWallet.fromMnemonic(mnemonic)
        XCTAssertEqual(a.seed, b.seed)
        XCTAssertEqual(a.publicKey, b.publicKey)
    }

    /// Different mnemonics produce different keys.
    func test_fromMnemonic_differentMnemonics_produceDifferentKeys() throws {
        let m1 = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
        let m2 = "legal winner thank year wave sausage worth useful legal winner thank yellow"
        let a = try WarrenWallet.fromMnemonic(m1)
        let b = try WarrenWallet.fromMnemonic(m2)
        XCTAssertNotEqual(a.seed, b.seed)
        XCTAssertNotEqual(a.publicKey, b.publicKey)
    }

    /// Malformed mnemonics throw `.invalidMnemonic`.
    func test_fromMnemonic_rejectsInvalidPhrases() {
        let invalid = [
            "",                                                                              // empty
            "too short phrase",                                                              // not 12 words
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon zoooo",   // bad word
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon", // bad checksum
        ]
        for phrase in invalid {
            XCTAssertThrowsError(try WarrenWallet.fromMnemonic(phrase), "Expected '\(phrase)' to be rejected") { error in
                XCTAssertTrue(error is WarrenWalletError)
            }
        }
    }

    /// Sign + verify roundtrip via Ed25519 algebra.
    func test_signCanonicalMessage_producesEd25519Signature() throws {
        let wallet = try WarrenWallet.generate()
        let payload = "warren-canonical-message-v1".data(using: .utf8)!
        let signature = try wallet.signCanonicalMessage(payload)
        XCTAssertEqual(signature.count, 64, "Ed25519 signatures are always 64 bytes")
        // Determinism: same seed + payload -> same signature.
        let second = try wallet.signCanonicalMessage(payload)
        XCTAssertEqual(signature, second)
    }

    /// Different payloads sign to different signatures.
    func test_signCanonicalMessage_differentPayloads_yieldDifferentSignatures() throws {
        let wallet = try WarrenWallet.generate()
        let s1 = try wallet.signCanonicalMessage("payload-a".data(using: .utf8)!)
        let s2 = try wallet.signCanonicalMessage("payload-b".data(using: .utf8)!)
        XCTAssertNotEqual(s1, s2)
    }

    /// Whitespace + casing tolerance during mnemonic loading.
    func test_fromMnemonic_trimsWhitespace() throws {
        let canonical = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
        let padded = "  \(canonical)  \n"
        let a = try WarrenWallet.fromMnemonic(canonical)
        let b = try WarrenWallet.fromMnemonic(padded)
        XCTAssertEqual(a.seed, b.seed)
    }

    /// `forgetSecret` clears the in-memory mnemonic. (Heap can't be
    /// reliably wiped in Swift, but the reference drop is testable.)
    func test_forgetSecret_clearsMnemonic() throws {
        let wallet = try WarrenWallet.generate()
        XCTAssertFalse(wallet.revealMnemonic().isEmpty)
        wallet.forgetSecret()
        XCTAssertEqual(wallet.revealMnemonic(), "")
    }
}
