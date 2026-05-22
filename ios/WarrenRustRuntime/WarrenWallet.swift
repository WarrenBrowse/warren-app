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
    /// The BIP39 mnemonic in cleartext. Hold only for the duration of
    /// the backup-display step ; zero out via `forgetSecret()` once
    /// persisted to the Keychain by the consumer.
    private(set) public var mnemonic: String
    /// 32-byte HKDF-derived Ed25519 seed.
    private(set) public var seed: Data
    /// 32-byte Ed25519 public key.
    public let publicKey: Data

    private init(mnemonic: String, seed: Data, publicKey: Data) {
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

    /// Loads a wallet from an existing 12-word BIP39 mnemonic, validating
    /// the phrase against the BIP39 wordlist + checksum.
    public static func fromMnemonic(_ mnemonic: String) throws -> WarrenWallet {
        let trimmed = mnemonic.trimmingCharacters(in: .whitespacesAndNewlines)
        // 32-byte seed buffer (filled by FFI on success).
        var seedBuffer = [UInt8](repeating: 0, count: 32)
        let seedStatus: Int32 = trimmed.withCString { cstr in
            seedBuffer.withUnsafeMutableBufferPointer { ptr in
                warren_wallet_seed_from_mnemonic(cstr, ptr.baseAddress)
            }
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
            // Wipe the seed before throwing.
            for i in 0..<seedBuffer.count { seedBuffer[i] = 0 }
            throw WarrenWalletError.ffi(pubkeyStatus)
        }
        return WarrenWallet(
            mnemonic: trimmed,
            seed: Data(seedBuffer),
            publicKey: Data(pubkeyBuffer)
        )
    }

    /// Returns the underlying BIP39 phrase. Avoid leaking to UI without
    /// the blur+reveal pattern enforced by `WarrenMnemonicDisplayView`.
    public func revealMnemonic() -> String {
        mnemonic
    }

    /// Returns the public key as a lower-case hex string (exactly 64
    /// characters). Safe to share with support : the pubkey is
    /// non-secret per Ed25519 cryptography. Used by
    /// `WarrenWalletIdentityView`.
    public var publicKeyHex: String {
        publicKey.map { String(format: "%02x", $0) }.joined()
    }

    /// Wipes the in-memory mnemonic string. Idempotent. Call after the
    /// consumer has persisted the mnemonic to the Keychain.
    public func forgetSecret() {
        // Swift strings on the heap cannot be reliably zeroed (CoW +
        // small-string optimisation), but we can drop the reference.
        mnemonic = ""
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
