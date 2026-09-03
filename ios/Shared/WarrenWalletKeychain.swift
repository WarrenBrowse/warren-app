//
//  WarrenWalletKeychain.swift
//  WarrenVPN
//
//  Created by Warren on 2026-05-21.
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  Stores the user BIP39 mnemonic in the iOS Keychain with strict
//  device-local + biometrics-friendly attributes.
//

import Foundation
import Security
import WarrenRustRuntime

/// Errors emitted by `WarrenWalletKeychain`.
public enum WarrenWalletKeychainError: Error, Equatable {
    /// No wallet has been stored yet.
    case notFound
    /// The stored data is not valid UTF-8.
    case decodingFailed
    /// Wrapped raw OSStatus from the Security framework.
    case secStatus(OSStatus)
}

/// Wraps the iOS Keychain to persist a Warren BIP39 mnemonic.
///
/// Design choices:
/// - **Strictly device-local** : `kSecAttrAccessibleWhenUnlockedThisDeviceOnly`
///   forbids iCloud Keychain sync and migration on backup restore.
/// - **No `kSecAttrSynchronizable: true`** : prevents accidental iCloud
///   propagation if the user enables Keychain sync system-wide.
/// - **Service identifier** mirrors the app bundle ID for clarity in
///   third-party Keychain inspectors (e.g. Keychain Access on macOS
///   when the device is connected via Configurator).
///
/// Threading: the Security framework calls used here are blocking;
/// callers must wrap reads/writes in `Task.detached(priority: .userInitiated)`
/// when running on the main actor.
public struct WarrenWalletKeychain {
    /// Service identifier in the Keychain (kSecAttrService).
    public static let service = "com.warrenbrowse.vpn.ios.wallet"
    /// Account identifier inside the service (kSecAttrAccount).
    /// A future Warren multi-wallet model could expand this to per-wallet labels.
    public static let mnemonicAccount = "mnemonic"

    /// Saves `mnemonic` to the Keychain. Overwrites any existing entry.
    public static func save(mnemonic: String) throws {
        guard let data = mnemonic.data(using: .utf8) else {
            throw WarrenWalletKeychainError.decodingFailed
        }
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: mnemonicAccount,
        ]
        let attributes: [String: Any] = [
            kSecValueData as String: data,
            kSecAttrAccessible as String: kSecAttrAccessibleWhenUnlockedThisDeviceOnly,
        ]
        // Try update first (idempotent re-save), then add on notFound.
        let updateStatus = SecItemUpdate(query as CFDictionary, attributes as CFDictionary)
        if updateStatus == errSecSuccess { return }
        if updateStatus == errSecItemNotFound {
            var addQuery = query
            for (key, value) in attributes {
                addQuery[key] = value
            }
            let addStatus = SecItemAdd(addQuery as CFDictionary, nil)
            guard addStatus == errSecSuccess else {
                throw WarrenWalletKeychainError.secStatus(addStatus)
            }
            return
        }
        throw WarrenWalletKeychainError.secStatus(updateStatus)
    }

    /// Loads the mnemonic from the Keychain.
    public static func load() throws -> String {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: mnemonicAccount,
            kSecReturnData as String: kCFBooleanTrue!,
            kSecMatchLimit as String: kSecMatchLimitOne,
        ]
        var item: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &item)
        switch status {
        case errSecSuccess:
            guard let data = item as? Data,
                  let mnemonic = String(data: data, encoding: .utf8) else {
                throw WarrenWalletKeychainError.decodingFailed
            }
            return mnemonic
        case errSecItemNotFound:
            throw WarrenWalletKeychainError.notFound
        default:
            throw WarrenWalletKeychainError.secStatus(status)
        }
    }

    /// Returns `true` if a wallet entry exists in the Keychain.
    /// Does not return the mnemonic itself (no biometric prompt triggered).
    public static func exists() -> Bool {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: mnemonicAccount,
            kSecReturnData as String: kCFBooleanFalse!,
            kSecMatchLimit as String: kSecMatchLimitOne,
        ]
        let status = SecItemCopyMatching(query as CFDictionary, nil)
        return status == errSecSuccess
    }

    /// Removes the wallet entry from the Keychain, and with it the forum
    /// identity the wallet signed in under: that handle is this wallet's
    /// pairwise name, so a wallet restored later on the same device must never
    /// be shown it (every erase path, logout included, comes through here).
    /// Safe to call when no entry exists (returns silently on
    /// `errSecItemNotFound`).
    public static func delete() throws {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: mnemonicAccount,
        ]
        let status = SecItemDelete(query as CFDictionary)
        guard status == errSecSuccess || status == errSecItemNotFound else {
            throw WarrenWalletKeychainError.secStatus(status)
        }
        do {
            try WarrenForumIdentityStore.delete()
        } catch WarrenForumIdentityStoreError.secStatus(let forumStatus) {
            throw WarrenWalletKeychainError.secStatus(forumStatus)
        } catch {
            throw WarrenWalletKeychainError.secStatus(errSecInternalError)
        }
    }
}
