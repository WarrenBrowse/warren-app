//
//  WarrenWallet.swift
//  WarrenRustRuntime
//
//  Created by Warren on 2026-05-21.
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  Scaffold Swift wrapper for `warren_wallet_ffi` (Rust crate `warren-ios`,
//  see `warren-ios/src/warren_wallet_ffi.rs`). NOT yet wired to the actual
//  FFI exports — those land in the C.3 deep-step-2 / C.5 implementation
//  brief. This file defines the consumer-facing Swift API surface that
//  the UI scaffolds (`WarrenMnemonicInputView`, `WarrenMnemonicDisplayView`,
//  `OnboardingWizardCoordinator`) target.
//

import Foundation

/// Errors emitted by `WarrenWallet`.
public enum WarrenWalletError: Error, Equatable {
    /// BIP39 parsing failed (invalid word, bad length, bad checksum).
    case invalidMnemonic
    /// Generic FFI failure with a human-readable Rust-side message.
    case ffi(String)
}

/// High-level Swift facade over the warren-identity / warren-ios FFI.
///
/// Threading: all methods are CPU-bound and fast (<1 ms for any of
/// `generate`, `seedFrom`, `derivePubkey`). Safe to call on the main
/// actor for UI flows.
public struct WarrenWallet {
    private let mnemonic: String

    /// 32-byte HKDF-derived Ed25519 seed.
    public let seed: Data
    /// 32-byte Ed25519 public key.
    public let publicKey: Data

    private init(mnemonic: String, seed: Data, publicKey: Data) {
        self.mnemonic = mnemonic
        self.seed = seed
        self.publicKey = publicKey
    }

    /// Generates a new 12-word BIP39 mnemonic and derives the Warren
    /// identity (seed + pubkey).
    ///
    /// Scaffold: returns a hardcoded test phrase. Production wires
    /// `warren_wallet_generate_mnemonic()` from `warren_wallet_ffi`.
    public static func generate() throws -> WarrenWallet {
        // TODO C.3 deep step 2: call warren_wallet_generate_mnemonic() via FFI.
        let testPhrase = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
        return try fromMnemonic(testPhrase)
    }

    /// Loads a wallet from an existing 12-word BIP39 mnemonic, validating
    /// the phrase against the BIP39 wordlist + checksum.
    ///
    /// Scaffold: returns a wallet with placeholder seed/pubkey when the
    /// mnemonic has 12 whitespace-separated tokens. Production wires
    /// `warren_wallet_mnemonic_to_seed` + `warren_wallet_derive_pubkey`.
    public static func fromMnemonic(_ mnemonic: String) throws -> WarrenWallet {
        let trimmed = mnemonic.trimmingCharacters(in: .whitespacesAndNewlines)
        let words = trimmed.split(separator: " ")
        guard words.count == 12 else {
            throw WarrenWalletError.invalidMnemonic
        }
        // TODO C.3 deep step 2: call warren_wallet_mnemonic_to_seed via FFI
        // and warren_wallet_derive_pubkey via FFI.
        let placeholderSeed = Data(repeating: 0, count: 32)
        let placeholderPubkey = Data(repeating: 0, count: 32)
        return WarrenWallet(
            mnemonic: trimmed,
            seed: placeholderSeed,
            publicKey: placeholderPubkey
        )
    }

    /// Returns the underlying BIP39 phrase. Avoid leaking to UI without
    /// the blur+reveal pattern enforced by `WarrenMnemonicDisplayView`.
    public func revealMnemonic() -> String {
        mnemonic
    }

    /// Signs `payload` with the Ed25519 derived signing key.
    /// The signature is a 64-byte Ed25519 signature suitable for the
    /// `X-Warren-Signature` HTTP header (canonical message convention
    /// from `warren-api-client`, M4.H.C.PRE refactor).
    ///
    /// Scaffold: returns 64 zero bytes. Production wires
    /// `warren_wallet_sign_canonical_message` via FFI.
    public func signCanonicalMessage(_ payload: Data) throws -> Data {
        // TODO C.3 deep step 2: call warren_wallet_sign_canonical_message via FFI.
        _ = payload
        return Data(repeating: 0, count: 64)
    }
}
