//
//  WarrenForumIdentityStore.swift
//  WarrenRustRuntime
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  The forum identity an approved wallet login hands back, and its home in
//  the iOS Keychain. The handle is public on the forum; its link to this
//  wallet is the private part, which is why it sits in the Keychain (device
//  local, never synced) next to the wallet and leaves with it.
//

import Foundation
import Security

/// The forum identity the broker derives for this wallet: the pairwise
/// proquint handle (`lusab-babad-dovok`) and the digest slot the activity
/// badge indexes, absent when the allocator had no room. Mirrors the desktop
/// `ForumIdentity` and the Android `ForumIdentity`.
public struct WarrenForumIdentity: Equatable, Sendable {
    public let handle: String
    public let notifySlot: UInt32?

    public init(handle: String, notifySlot: UInt32?) {
        self.handle = handle
        self.notifySlot = notifySlot
    }
}

/// Errors emitted by `WarrenForumIdentityStore`.
public enum WarrenForumIdentityStoreError: Error, Equatable {
    /// The identity could not be serialised, or a stored item is not one.
    case encodingFailed
    /// Wrapped raw OSStatus from the Security framework.
    case secStatus(OSStatus)
}

/// Keychain-backed store of the wallet's forum identity, one item per
/// install. Same attributes as the wallet's own entry: device local,
/// `WhenUnlockedThisDeviceOnly`, never synchronised. Erasing the wallet erases
/// this item too (`WarrenWalletKeychain.delete`), so a new wallet never shows
/// the previous one's name.
///
/// The Security calls block; the store is written from the login's background
/// queue and read once by the account screen.
public enum WarrenForumIdentityStore {
    /// Service identifier in the Keychain (kSecAttrService).
    public static let service = "com.warrenbrowse.vpn.ios.forum"
    /// Account identifier inside the service (kSecAttrAccount).
    public static let identityAccount = "identity"

    /// Posted on the main queue after a save or a delete, so an account
    /// screen already on display refreshes its "Forum name" row.
    public static let didChangeNotification = Notification.Name("WarrenForumIdentityStoreDidChange")

    private static let handleKey = "handle"
    private static let notifySlotKey = "notify_slot"

    /// Stores `identity`, replacing any previous one.
    public static func save(_ identity: WarrenForumIdentity) throws {
        var object: [String: Any] = [handleKey: identity.handle]
        if let slot = identity.notifySlot {
            object[notifySlotKey] = NSNumber(value: slot)
        }
        guard let data = try? JSONSerialization.data(withJSONObject: object) else {
            throw WarrenForumIdentityStoreError.encodingFailed
        }
        let attributes: [String: Any] = [
            kSecValueData as String: data,
            kSecAttrAccessible as String: kSecAttrAccessibleWhenUnlockedThisDeviceOnly,
        ]
        let updateStatus = SecItemUpdate(query() as CFDictionary, attributes as CFDictionary)
        switch updateStatus {
        case errSecSuccess:
            break
        case errSecItemNotFound:
            var addQuery = query()
            for (key, value) in attributes {
                addQuery[key] = value
            }
            let addStatus = SecItemAdd(addQuery as CFDictionary, nil)
            guard addStatus == errSecSuccess else {
                throw WarrenForumIdentityStoreError.secStatus(addStatus)
            }
        default:
            throw WarrenForumIdentityStoreError.secStatus(updateStatus)
        }
        notifyChange()
    }

    /// The stored identity, or `nil` when none is stored or the item is not
    /// readable as one.
    public static func load() -> WarrenForumIdentity? {
        var readQuery = query()
        readQuery[kSecReturnData as String] = kCFBooleanTrue
        readQuery[kSecMatchLimit as String] = kSecMatchLimitOne
        var item: CFTypeRef?
        guard SecItemCopyMatching(readQuery as CFDictionary, &item) == errSecSuccess,
            let data = item as? Data,
            let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
            let handle = object[handleKey] as? String
        else {
            return nil
        }
        let slot = (object[notifySlotKey] as? NSNumber).map { UInt32(truncating: $0) }
        return WarrenForumIdentity(handle: handle, notifySlot: slot)
    }

    /// Removes the stored identity. A no-op when none is stored.
    public static func delete() throws {
        let status = SecItemDelete(query() as CFDictionary)
        guard status == errSecSuccess || status == errSecItemNotFound else {
            throw WarrenForumIdentityStoreError.secStatus(status)
        }
        notifyChange()
    }

    private static func query() -> [String: Any] {
        [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: identityAccount,
        ]
    }

    private static func notifyChange() {
        DispatchQueue.main.async {
            NotificationCenter.default.post(name: didChangeNotification, object: nil)
        }
    }
}
