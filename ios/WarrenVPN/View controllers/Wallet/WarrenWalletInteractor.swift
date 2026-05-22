//
//  WarrenWalletInteractor.swift
//  WarrenVPN
//
//  Created by Warren on 2026-05-21.
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  Business logic for the Warren wallet flow. Bridges between the
//  Swift UI layer (via Coordinators + ViewControllers) and the
//  Rust-backed FFI (`WarrenWallet` in WarrenRustRuntime) + iOS Keychain
//  (`WarrenWalletKeychain`). Keeps view controllers thin.
//

import Foundation
import LocalAuthentication
import WarrenLogging
import WarrenRustRuntime

/// Errors emitted by `WarrenWalletInteractor`.
public enum WarrenWalletInteractorError: Error, Equatable {
    /// BIP39 phrase failed to parse / verify.
    case invalidMnemonic
    /// Generation failed (RNG / FFI).
    case generationFailed
    /// Keychain operation failed.
    case keychain(WarrenWalletKeychainError)
    /// Biometric/passcode authentication denied or unavailable.
    case authenticationFailed(String)
    /// No wallet has been provisioned yet.
    case noWallet

    public static func == (lhs: WarrenWalletInteractorError, rhs: WarrenWalletInteractorError) -> Bool {
        switch (lhs, rhs) {
        case (.invalidMnemonic, .invalidMnemonic),
            (.generationFailed, .generationFailed),
            (.noWallet, .noWallet):
            return true
        case (.keychain(let a), .keychain(let b)):
            return a == b
        case (.authenticationFailed(let a), .authenticationFailed(let b)):
            return a == b
        default:
            return false
        }
    }
}

/// Business-logic facade for the Warren wallet lifecycle. All async
/// methods are `Sendable`-safe and execute on a non-main queue to avoid
/// blocking the UI ; results are surfaced via `@MainActor`-isolated
/// completion handlers.
public final class WarrenWalletInteractor: @unchecked Sendable {
    private let logger = Logger(label: "WarrenWalletInteractor")
    private let queue = DispatchQueue(label: "WarrenWalletInteractor", qos: .userInitiated)

    public init() {}

    /// Returns true if a wallet entry exists in the Keychain. Safe to
    /// call from the main actor (does not trigger biometric prompt).
    /// Synchronously loads the wallet pubkey as a hex string from the
    /// Keychain. Returns `nil` if no wallet is present or the Keychain
    /// entry is unreadable.
    ///
    /// Public key is non-secret per Ed25519 cryptography ; no
    /// biometric prompt is required (in contrast to
    /// `loadMnemonicWithAuth(reason:completion:)` which gates on
    /// Face ID / Touch ID because the mnemonic IS secret). The seed
    /// material is zeroed via `forgetSecret()` immediately after
    /// derivation.
    public func publicKeyHex() -> String? {
        guard let mnemonic = try? WarrenWalletKeychain.load(),
            let wallet = try? WarrenWallet.fromMnemonic(mnemonic)
        else {
            return nil
        }
        let hex = wallet.publicKeyHex
        wallet.forgetSecret()
        return hex
    }

    /// Convenience : `<first 4 hex>...<last 4 hex>` short form of
    /// the wallet pubkey. Used in compact UI surfaces (Diagnostic
    /// info row, App Group status) where the full 64-char hex would
    /// wrap to multiple lines. Returns `nil` when no wallet exists.
    public func publicKeyShort() -> String? {
        guard let hex = publicKeyHex(), hex.count >= 8 else { return nil }
        return "\(hex.prefix(4))...\(hex.suffix(4))"
    }

    public func walletExists() -> Bool {
        WarrenWalletKeychain.exists()
    }

    /// Generates a new 12-word BIP39 mnemonic via the Rust FFI.
    /// Does NOT persist ; caller decides whether to save after user
    /// confirms the backup step.
    public func generateMnemonic(
        completion: @escaping @Sendable @MainActor (Result<String, WarrenWalletInteractorError>) -> Void
    ) {
        queue.async { [weak self] in
            do {
                let wallet = try WarrenWallet.generate()
                let mnemonic = wallet.revealMnemonic()
                Task { @MainActor in
                    completion(.success(mnemonic))
                }
            } catch {
                self?.logger.error("generateMnemonic failed: \(error)")
                Task { @MainActor in
                    completion(.failure(.generationFailed))
                }
            }
        }
    }

    /// Validates an existing mnemonic + persists it to the Keychain.
    public func importMnemonic(
        _ mnemonic: String,
        completion: @escaping @Sendable @MainActor (Result<Void, WarrenWalletInteractorError>) -> Void
    ) {
        queue.async { [weak self] in
            do {
                _ = try WarrenWallet.fromMnemonic(mnemonic)
                try WarrenWalletKeychain.save(mnemonic: mnemonic)
                Task { @MainActor in
                    completion(.success(()))
                }
            } catch let kcErr as WarrenWalletKeychainError {
                self?.logger.error("importMnemonic Keychain error: \(kcErr)")
                Task { @MainActor in
                    completion(.failure(.keychain(kcErr)))
                }
            } catch {
                self?.logger.error("importMnemonic invalid: \(error)")
                Task { @MainActor in
                    completion(.failure(.invalidMnemonic))
                }
            }
        }
    }

    /// Persists a freshly generated mnemonic to the Keychain.
    public func saveMnemonic(
        _ mnemonic: String,
        completion: @escaping @Sendable @MainActor (Result<Void, WarrenWalletInteractorError>) -> Void
    ) {
        queue.async { [weak self] in
            do {
                try WarrenWalletKeychain.save(mnemonic: mnemonic)
                Task { @MainActor in
                    completion(.success(()))
                }
            } catch let kcErr as WarrenWalletKeychainError {
                self?.logger.error("saveMnemonic Keychain error: \(kcErr)")
                Task { @MainActor in
                    completion(.failure(.keychain(kcErr)))
                }
            } catch {
                Task { @MainActor in
                    completion(.failure(.keychain(.secStatus(-1))))
                }
            }
        }
    }

    /// Loads the wallet mnemonic from the Keychain after biometric or
    /// passcode authentication.
    ///
    /// `reason` is shown in the Face ID / Touch ID system prompt and
    /// MUST be localized by the caller.
    public func loadMnemonicWithAuth(
        reason: String,
        completion: @escaping @Sendable @MainActor (Result<String, WarrenWalletInteractorError>) -> Void
    ) {
        let context = LAContext()
        var error: NSError?
        let policy: LAPolicy = .deviceOwnerAuthentication
        guard context.canEvaluatePolicy(policy, error: &error) else {
            let message = error?.localizedDescription ?? "Authentication unavailable"
            Task { @MainActor in
                completion(.failure(.authenticationFailed(message)))
            }
            return
        }
        context.evaluatePolicy(policy, localizedReason: reason) { [weak self] success, evalError in
            guard let self else { return }
            if success {
                self.queue.async {
                    do {
                        let mnemonic = try WarrenWalletKeychain.load()
                        Task { @MainActor in
                            completion(.success(mnemonic))
                        }
                    } catch WarrenWalletKeychainError.notFound {
                        Task { @MainActor in
                            completion(.failure(.noWallet))
                        }
                    } catch let kcErr as WarrenWalletKeychainError {
                        Task { @MainActor in
                            completion(.failure(.keychain(kcErr)))
                        }
                    } catch {
                        Task { @MainActor in
                            completion(.failure(.keychain(.secStatus(-1))))
                        }
                    }
                }
            } else {
                let message = evalError?.localizedDescription ?? "Authentication denied"
                Task { @MainActor in
                    completion(.failure(.authenticationFailed(message)))
                }
            }
        }
    }

    /// Removes the wallet from the Keychain. Use carefully : irreversible
    /// without the BIP39 backup phrase.
    public func forgetWallet(
        completion: @escaping @Sendable @MainActor (Result<Void, WarrenWalletInteractorError>) -> Void
    ) {
        queue.async { [weak self] in
            do {
                try WarrenWalletKeychain.delete()
                Task { @MainActor in
                    completion(.success(()))
                }
            } catch let kcErr as WarrenWalletKeychainError {
                self?.logger.error("forgetWallet Keychain error: \(kcErr)")
                Task { @MainActor in
                    completion(.failure(.keychain(kcErr)))
                }
            } catch {
                Task { @MainActor in
                    completion(.failure(.keychain(.secStatus(-1))))
                }
            }
        }
    }
}
