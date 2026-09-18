//
//  WarrenWallet.swift
//  WarrenRustRuntime
//
//  Created by Warren on 2026-05-21.
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  Swift facade over the `warren_wallet_ffi` Rust exports
//  (`warren-ios/src/warren_wallet_ffi.rs`). Wraps the C ABI with an
//  idiomatic Swift API + ensures every secret buffer is zeroed before
//  drop. The underlying crypto is provided by `warren-identity`
//  (warren-core) which uses BIP39 v2 + Ed25519-dalek v2 + HKDF-SHA256
//  with frozen domain separation constants.
//

import Foundation
import WarrenRustRuntimeProxy

/// Errors emitted by `WarrenWallet`.
public enum WarrenWalletError: Error, Equatable {
    /// BIP39 parsing failed (invalid word, bad length, bad checksum).
    case invalidMnemonic
    /// RNG / FFI failure during BIP39 generation.
    case generationFailed
    /// Underlying Rust FFI returned a non-zero status.
    case ffi(Int32)
}

/// High-level Swift facade over the warren-identity / warren-ios FFI.
///
/// Memory model : the 32-byte seed is held in a `Data` instance and
/// explicitly zeroed in `deinit`. Callers MUST NOT copy `seed` to
/// another long-lived `Data` ; instead, pass `seed` directly to
/// `signCanonicalMessage(_:)` which keeps the secret inside Rust.
public final class WarrenWallet {
    /// The BIP39 mnemonic in cleartext, for the generate-and-show-once flow
    /// that is the only place a wallet is created from a phrase nobody has
    /// stored yet. Nil for a wallet derived from a `WarrenSecureMnemonic`,
    /// which is how every signing flow loads one: a `String` cannot be wiped,
    /// so a wallet used only to sign must not carry one.
    private(set) public var mnemonic: String?
    /// 32-byte HKDF-derived Ed25519 seed.
    private(set) public var seed: Data
    /// 32-byte Ed25519 public key.
    public let publicKey: Data

    private init(mnemonic: String?, seed: Data, publicKey: Data) {
        self.mnemonic = mnemonic
        self.seed = seed
        self.publicKey = publicKey
    }

    deinit {
        // Best-effort wipe of secret material. Swift does not guarantee
        // that other heap copies are cleared, so the Rust side already
        // zeroizes via `Zeroizing<[u8; 32]>` and we mirror that here.
        let count = seed.count
        if count > 0 {
            seed.withUnsafeMutableBytes { buffer in
                memset_s(buffer.baseAddress, count, 0, count)
            }
        }
    }

    /// Generates a new 12-word BIP39 mnemonic and derives the Warren
    /// identity (seed + pubkey).
    public static func generate() throws -> WarrenWallet {
        guard let cstr = warren_wallet_generate_mnemonic(12) else {
            throw WarrenWalletError.generationFailed
        }
        defer { warren_wallet_free_mnemonic(cstr) }
        let phrase = String(cString: cstr)
        return try fromMnemonic(phrase)
    }

    /// Loads a wallet from a phrase held in a buffer that can be wiped.
    ///
    /// The one path a signing flow should take: the phrase goes Keychain to
    /// Rust without ever becoming a `String`, which cannot be erased. The
    /// wallet it returns holds no phrase for the same reason, so the caller
    /// keeps the `WarrenSecureMnemonic` for as long as it needs the words and
    /// wipes it.
    public static func fromMnemonic(_ mnemonic: WarrenSecureMnemonic) throws -> WarrenWallet {
        try mnemonic.withCString { cstr in
            try derive(from: cstr, phrase: nil)
        }
    }

    /// Loads a wallet from an existing 12-word BIP39 mnemonic, validating
    /// the phrase against the BIP39 wordlist + checksum.
    public static func fromMnemonic(_ mnemonic: String) throws -> WarrenWallet {
        let trimmed = mnemonic.trimmingCharacters(in: .whitespacesAndNewlines)
        return try trimmed.withCString { cstr in
            try derive(from: cstr, phrase: trimmed)
        }
    }

    /// Seed and pubkey from a phrase already in C form, the one derivation
    /// both entries share.
    private static func derive(
        from cstr: UnsafePointer<CChar>,
        phrase: String?
    ) throws -> WarrenWallet {
        // 32-byte seed buffer (filled by FFI on success).
        var seedBuffer = [UInt8](repeating: 0, count: 32)
        // Every exit wipes it, including the successful one, which used to
        // walk out leaving the seed in the buffer. `Data(seedBuffer)` below is
        // already its own copy by the time this runs. `memset_s` rather than a
        // loop: a loop writing bytes nobody reads again is exactly what a
        // compiler is allowed to delete.
        defer {
            seedBuffer.withUnsafeMutableBufferPointer { buffer in
                memset_s(buffer.baseAddress, buffer.count, 0, buffer.count)
            }
        }
        let seedStatus: Int32 = seedBuffer.withUnsafeMutableBufferPointer { ptr in
            warren_wallet_seed_from_mnemonic(cstr, ptr.baseAddress)
        }
        guard seedStatus == 0 else {
            throw WarrenWalletError.invalidMnemonic
        }
        // Derive pubkey from seed (32 bytes).
        var pubkeyBuffer = [UInt8](repeating: 0, count: 32)
        let pubkeyStatus: Int32 = seedBuffer.withUnsafeBufferPointer { seedPtr in
            pubkeyBuffer.withUnsafeMutableBufferPointer { pubPtr in
                warren_wallet_derive_pubkey(seedPtr.baseAddress, pubPtr.baseAddress)
            }
        }
        guard pubkeyStatus == 0 else {
            throw WarrenWalletError.ffi(pubkeyStatus)
        }
        return WarrenWallet(
            mnemonic: phrase,
            seed: Data(seedBuffer),
            publicKey: Data(pubkeyBuffer)
        )
    }

    /// Returns the underlying BIP39 phrase. Avoid leaking to UI without
    /// the blur+reveal pattern enforced by `WarrenMnemonicDisplayView`.
    public func revealMnemonic() -> String {
        mnemonic ?? ""
    }

    /// Returns the public key as a lower-case hex string (exactly 64
    /// characters). Retained for low-level diagnostics and byte-level
    /// round-trip checks ; the user-facing identity is the SS58
    /// `publicKeyAddress` below. The pubkey is non-secret per Ed25519
    /// cryptography.
    public var publicKeyHex: String {
        publicKey.map { String(format: "%02x", $0) }.joined()
    }

    /// Returns the canonical **Warren SS58 address** (`wb…`, network
    /// prefix 13295) derived from the wallet pubkey. This is the
    /// identity shown in the UI, copied to the clipboard, and carried in
    /// the `X-Warren-PubKey` header. Computed authoritatively by the
    /// Rust layer (`warren_wallet_pubkey_ss58` → `warren_identity::ss58`)
    /// so it round-trips byte-for-byte with the daemon and the backend.
    /// Safe to share with support : the address cannot be used to access
    /// the wallet or sign on its behalf.
    public var publicKeyAddress: String {
        seed.withUnsafeBytes { seedRaw -> String in
            guard let base = seedRaw.bindMemory(to: UInt8.self).baseAddress,
                let cstr = warren_wallet_pubkey_ss58(base)
            else {
                return ""
            }
            defer { warren_wallet_free_mnemonic(cstr) }
            return String(cString: cstr)
        }
    }

    /// Drops the phrase and zeroes the seed. Idempotent. Call as soon as the
    /// consumer has persisted the phrase to the Keychain, or finished
    /// signing.
    ///
    /// The seed is wiped for real; the phrase, when this wallet carries one at
    /// all, can only be dropped, which is why every signing flow loads through
    /// `WarrenSecureMnemonic` and this wallet then holds none.
    public func forgetSecret() {
        mnemonic = nil
        let count = seed.count
        guard count > 0 else { return }
        seed.withUnsafeMutableBytes { buffer in
            memset_s(buffer.baseAddress, count, 0, count)
        }
    }

    /// Signs `payload` with the Ed25519 derived signing key.
    /// The signature is a 64-byte Ed25519 signature suitable for the
    /// `X-Warren-Signature` HTTP header (canonical message convention
    /// from `warren-api-client`).
    public func signCanonicalMessage(_ payload: Data) throws -> Data {
        var signatureBuffer = [UInt8](repeating: 0, count: 64)
        let status: Int32 = seed.withUnsafeBytes { seedRaw in
            payload.withUnsafeBytes { payloadRaw in
                signatureBuffer.withUnsafeMutableBufferPointer { sigPtr in
                    warren_wallet_sign(
                        seedRaw.bindMemory(to: UInt8.self).baseAddress,
                        payloadRaw.bindMemory(to: UInt8.self).baseAddress,
                        UInt(payload.count),
                        sigPtr.baseAddress
                    )
                }
            }
        }
        guard status == 0 else {
            throw WarrenWalletError.ffi(status)
        }
        return Data(signatureBuffer)
    }
}
