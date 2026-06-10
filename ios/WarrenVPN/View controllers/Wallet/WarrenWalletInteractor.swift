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
    /// A warren-api account call (subscription / voucher / delete)
    /// failed. Carries the underlying transport / server error.
    case account(WarrenAccountError)

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
        case (.account(let a), .account(let b)):
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

    /// Synchronously loads the wallet's canonical **Warren SS58
    /// address** (`wb…`) from the Keychain. Returns `nil` if no wallet
    /// is present or the Keychain entry is unreadable. This is the
    /// user-facing identity (display, copy, support) ; for the raw hex
    /// form use `publicKeyHex()`.
    ///
    /// The public key is non-secret per Ed25519 cryptography ; no
    /// biometric prompt is required (in contrast to
    /// `loadMnemonicWithAuth(reason:completion:)` which gates on
    /// Face ID / Touch ID because the mnemonic IS secret). The seed
    /// material is zeroed via `forgetSecret()` immediately after
    /// derivation.
    public func publicKeyAddress() -> String? {
        guard let mnemonic = try? WarrenWalletKeychain.load(),
            let wallet = try? WarrenWallet.fromMnemonic(mnemonic)
        else {
            return nil
        }
        let address = wallet.publicKeyAddress
        wallet.forgetSecret()
        return address.isEmpty ? nil : address
    }

    /// Synchronously loads the wallet pubkey as a lower-case hex string
    /// from the Keychain. Returns `nil` if no wallet is present or the
    /// Keychain entry is unreadable. Retained for low-level diagnostics ;
    /// prefer `publicKeyAddress()` for anything user-facing.
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

    /// Convenience : `wb7kgy…hP9DnB` short form of the wallet SS58
    /// address (first 6 + `…` + last 6). Used in compact UI surfaces
    /// (Diagnostic info row, App Group status) where the full address
    /// would wrap. Returns `nil` when no wallet exists. The full
    /// address - not this short form - must be used for copy / share.
    public func publicKeyShort() -> String? {
        guard let address = publicKeyAddress() else { return nil }
        return address.shortWarrenAddress
    }

    /// Returns true if a wallet entry exists in the Keychain. Safe to
    /// call from the main actor (does not trigger a biometric prompt).
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

    /// Fetches the wallet's subscription expiry from warren-api (signed
    /// `GET /v1/subscription`), the Warren-native replacement for the
    /// Mullvad account-expiry REST call. A wallet with no bound
    /// subscription (HTTP 404) resolves to the Unix epoch, which the
    /// account chrome renders as "out of time" (aligned with the desktop
    /// `account-data-cache` no-subscription treatment).
    ///
    /// Runs off the main thread and loads the seed silently: the wallet
    /// Keychain item is `WhenUnlockedThisDeviceOnly` (not biometric
    /// gated), matching the tunnel which signs with the same key
    /// continuously. The seed never leaves this queue.
    public func fetchSubscriptionExpiry(
        completion: @escaping @Sendable (Result<Date, WarrenWalletInteractorError>) -> Void
    ) {
        queue.async { [weak self] in
            guard let mnemonic = try? WarrenWalletKeychain.load(),
                let wallet = try? WarrenWallet.fromMnemonic(mnemonic)
            else {
                completion(.failure(.noWallet))
                return
            }
            defer { wallet.forgetSecret() }
            switch WarrenAccountClient.subscription(seed: wallet.seed) {
            case let .success(.active(expiry)):
                completion(.success(expiry))
            case .success(.none):
                completion(.success(Date(timeIntervalSince1970: 0)))
            case let .failure(error):
                self?.logger.error("subscription fetch failed: \(error)")
                completion(.failure(.account(error)))
            }
        }
    }

    /// Redeems a subscription voucher against warren-api (unsigned
    /// `POST /v1/register`), binding this wallet's pubkey to a new
    /// subscription. Returns the new expiry. The Warren-native
    /// replacement for the Mullvad voucher submission. The voucher code
    /// is never logged.
    public func redeemVoucher(
        code: String,
        completion: @escaping @Sendable (Result<Date, WarrenWalletInteractorError>) -> Void
    ) {
        queue.async { [weak self] in
            guard let mnemonic = try? WarrenWalletKeychain.load(),
                let wallet = try? WarrenWallet.fromMnemonic(mnemonic)
            else {
                completion(.failure(.noWallet))
                return
            }
            defer { wallet.forgetSecret() }
            switch WarrenAccountClient.redeemVoucher(seed: wallet.seed, code: code) {
            case let .success(expiry):
                completion(.success(expiry))
            case let .failure(error):
                self?.logger.error("voucher redemption failed: \(error)")
                completion(.failure(.account(error)))
            }
        }
    }

    /// Deletes the wallet's subscription server-side (signed
    /// `DELETE /v1/account`). Does NOT touch the local Keychain wallet;
    /// callers wipe the wallet separately via `forgetWallet`.
    public func deleteServerAccount(
        completion: @escaping @Sendable (Result<Void, WarrenWalletInteractorError>) -> Void
    ) {
        queue.async { [weak self] in
            guard let mnemonic = try? WarrenWalletKeychain.load(),
                let wallet = try? WarrenWallet.fromMnemonic(mnemonic)
            else {
                completion(.failure(.noWallet))
                return
            }
            defer { wallet.forgetSecret() }
            switch WarrenAccountClient.deleteAccount(seed: wallet.seed) {
            case .success:
                completion(.success(()))
            case let .failure(error):
                self?.logger.error("account deletion failed: \(error)")
                completion(.failure(.account(error)))
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
