//
//  WireGuardKey.swift
//  WarrenRustRuntime
//
//  Originally created by Emils on 2026-04-07 for the Mullvad fork
//  (used the Rust `mullvad_generate_private_key` + `mullvad_derive_public_key`
//  FFI symbols, both of which are NOT exported by Warren's `warren-ios`
//  staticlib - Warren tunnels via Quinn over Ed25519 wallet identity,
//  not WireGuard X25519).
//
//  Reimplemented (2026-05-22, C.4.5) on top of `CryptoKit.Curve25519`
//  so it stays pure Swift (no Rust FFI dependency) yet still produces
//  WireGuard-compatible X25519 keypairs for the legacy code paths
//  that consume `WireGuard.PrivateKey()` + `.publicKey` (Mullvad
//  device-key flows + REST `Device.pubkey` field).
//
//  Long-term : the Mullvad device-pubkey contract is replaced by the
//  Warren Ed25519 wallet pubkey ; this file then becomes unused and
//  can be deleted alongside the WireGuard namespace.
//

import CryptoKit
import Foundation
import WarrenTypes

private let keyLength = 32

extension WireGuard.PrivateKey {
    /// Generate a new random X25519 private key via Apple's
    /// `CryptoKit.Curve25519.KeyAgreement`. Drop-in replacement for
    /// the previous Rust FFI call ; same wire format (32 bytes,
    /// little-endian X25519 scalar) so persisted keys remain
    /// compatible with the WireGuardKit serialization.
    public init() {
        let cryptoKey = Curve25519.KeyAgreement.PrivateKey()
        let data = cryptoKey.rawRepresentation
        // CryptoKit always returns 32 bytes ; defensive assertion
        // against future ABI changes. Force-unwrap the failable
        // `init?(rawValue:)` since we just validated the length.
        guard data.count == 32, let key = WireGuard.PrivateKey(rawValue: data) else {
            preconditionFailure("Curve25519 keypair did not produce 32 bytes - CryptoKit ABI change")
        }
        self = key
    }

    /// Derive the corresponding X25519 public key via CryptoKit.
    /// Drop-in replacement for the previous Rust FFI call.
    public var publicKey: WireGuard.PublicKey {
        guard let key = try? Curve25519.KeyAgreement.PrivateKey(rawRepresentation: rawValue) else {
            // The raw bytes came out of a prior `init()` or REST
            // payload that already validated the 32-byte length ; a
            // CryptoKit rejection here means a corrupt persisted key.
            return WireGuard.PublicKey(rawValue: Data(repeating: 0, count: keyLength))!
        }
        let pubData = key.publicKey.rawRepresentation
        return WireGuard.PublicKey(rawValue: pubData)!
    }
}
