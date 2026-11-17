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
    /// The bundle names no App Group, so the seed the tunnel extension reads
    /// has nowhere to go.
    case sharedGroupUnavailable
}

/// Wraps the iOS Keychain to persist a Warren BIP39 mnemonic.
///
/// Design choices:
/// - **Strictly device-local** : `kSecAttrAccessibleWhenUnlockedThisDeviceOnly`
///   forbids iCloud Keychain sync and migration on backup restore.
/// - **No `kSecAttrSynchronizable: true`** : prevents accidental iCloud
///   propagation if the user enables Keychain sync system-wide.
/// - **Private to the app** : the phrase item names no access group, so it
///   lands in the app's own default group. The packet tunnel extension
///   reads the seed item of `WarrenTunnelSeedKeychain`, never the phrase.
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
        try save(data: data)
    }

    /// The phrase, then the seed the tunnel extension reads (see
    /// `WarrenTunnelSeedKeychain`). Every save goes through here, so the two
    /// items always describe the same wallet. The seed is derived first: a
    /// phrase that is not a wallet is refused before anything is written.
    private static func save(data: Data) throws {
        let group = try sharedGroup()
        let wallet = try WarrenWallet.fromMnemonic(WarrenSecureMnemonic(data: data))
        defer { wallet.forgetSecret() }
        try savePhrase(data: data)
        try WarrenTunnelSeedKeychain.save(seed: wallet.seed, accessGroup: group)
    }

    private static func saveTunnelSeed(phrase: Data) throws {
        let group = try sharedGroup()
        let wallet = try WarrenWallet.fromMnemonic(WarrenSecureMnemonic(data: phrase))
        defer { wallet.forgetSecret() }
        try WarrenTunnelSeedKeychain.save(seed: wallet.seed, accessGroup: group)
    }

    private static func sharedGroup() throws -> String {
        guard let group = WarrenTunnelSeedKeychain.bundleAccessGroup else {
            throw WarrenWalletKeychainError.sharedGroupUnavailable
        }
        return group
    }

    private static func savePhrase(data: Data) throws {
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

    /// Loads the phrase into a buffer that can be wiped, which a `String`
    /// cannot be. Every caller that only passes the phrase on to Rust uses
    /// this; [`load()`] stays for the one screen that has to draw it.
    public static func loadSecure() throws -> WarrenSecureMnemonic {
        WarrenSecureMnemonic(data: try loadData())
    }

    /// Saves `mnemonic` without ever making a `String` of it.
    public static func save(mnemonic: WarrenSecureMnemonic) throws {
        try mnemonic.withData { try save(data: $0) }
    }

    /// Loads the mnemonic as text. Reserved for the backup screen, which has
    /// to draw it; everything else takes [`loadSecure()`].
    public static func load() throws -> String {
        guard let mnemonic = String(data: try loadData(), encoding: .utf8) else {
            throw WarrenWalletKeychainError.decodingFailed
        }
        return mnemonic
    }

    /// The stored bytes, the one read both loaders share.
    private static func loadData() throws -> Data {
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
            guard let data = item as? Data else {
                throw WarrenWalletKeychainError.decodingFailed
            }
            return data
        case errSecItemNotFound:
            throw WarrenWalletKeychainError.notFound
        default:
            throw WarrenWalletKeychainError.secStatus(status)
        }
    }

    /// Writes the tunnel extension's seed item for a wallet saved before that
    /// item existed. A no-op without a wallet, or when the item is already
    /// there; the app calls it at every launch.
    public static func provisionTunnelSeedIfMissing() throws {
        let group = try sharedGroup()
        do {
            _ = try WarrenTunnelSeedKeychain.load(accessGroup: group)
            return
        } catch WarrenWalletKeychainError.notFound {}
        let phrase: Data
        do {
            phrase = try loadData()
        } catch WarrenWalletKeychainError.notFound {
            return
        }
        try saveTunnelSeed(phrase: phrase)
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
        if let group = WarrenTunnelSeedKeychain.bundleAccessGroup {
            try WarrenTunnelSeedKeychain.delete(accessGroup: group)
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

/// The 32-byte wallet seed, the one wallet secret the packet tunnel extension
/// needs.
///
/// The extension runs under its own App ID, so the phrase item above, stored
/// in the app's default access group, is out of its reach. It reads this item
/// instead, stored in the App Group's access group (which both entitlements
/// carry) and readable after the first unlock, because an on-demand rule can
/// start the tunnel while the device is locked. The phrase never leaves the
/// app's private group.
public enum WarrenTunnelSeedKeychain {
    /// Account identifier of the seed item, under `WarrenWalletKeychain.service`.
    public static let account = "tunnel-seed"

    /// The App Group of the running bundle, which is also the keychain access
    /// group both processes share. `nil` outside a configured bundle.
    public static var bundleAccessGroup: String? {
        Bundle.main.object(forInfoDictionaryKey: "ApplicationSecurityGroupIdentifier") as? String
    }

    public static func save(seed: Data, accessGroup: String) throws {
        let query = baseQuery(accessGroup: accessGroup)
        let attributes: [String: Any] = [
            kSecValueData as String: seed,
            kSecAttrAccessible as String: kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly,
        ]
        let updateStatus = SecItemUpdate(query as CFDictionary, attributes as CFDictionary)
        if updateStatus == errSecSuccess { return }
        guard updateStatus == errSecItemNotFound else {
            throw WarrenWalletKeychainError.secStatus(updateStatus)
        }
        let addStatus = SecItemAdd(query.merging(attributes) { $1 } as CFDictionary, nil)
        guard addStatus == errSecSuccess else {
            throw WarrenWalletKeychainError.secStatus(addStatus)
        }
    }

    public static func load(accessGroup: String) throws -> Data {
        var query = baseQuery(accessGroup: accessGroup)
        query[kSecReturnData as String] = kCFBooleanTrue!
        query[kSecMatchLimit as String] = kSecMatchLimitOne
        var item: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &item)
        switch status {
        case errSecSuccess:
            guard let data = item as? Data, data.count == 32 else {
                throw WarrenWalletKeychainError.decodingFailed
            }
            return data
        case errSecItemNotFound:
            throw WarrenWalletKeychainError.notFound
        default:
            throw WarrenWalletKeychainError.secStatus(status)
        }
    }

    /// Safe to call when no item exists.
    public static func delete(accessGroup: String) throws {
        let status = SecItemDelete(baseQuery(accessGroup: accessGroup) as CFDictionary)
        guard status == errSecSuccess || status == errSecItemNotFound else {
            throw WarrenWalletKeychainError.secStatus(status)
        }
    }

    private static func baseQuery(accessGroup: String) -> [String: Any] {
        [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: WarrenWalletKeychain.service,
            kSecAttrAccount as String: account,
            kSecAttrAccessGroup as String: accessGroup,
        ]
    }
}
