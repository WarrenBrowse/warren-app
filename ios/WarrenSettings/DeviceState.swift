//
//  DeviceState.swift
//  MullvadVPN
//
//  Created by Marco Nikic on 2023-07-31.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import Foundation
import WarrenTypes

public enum DeviceState: Codable, Equatable, Sendable {
    case loggedIn(StoredAccountData, StoredDeviceData)
    case loggedOut
    case revoked

    /// Builds a logged-in state from a Warren wallet identity. The account
    /// number carries the SS58 address and the expiry is open-ended because
    /// Warren enforces subscription exit-side, not via a client expiry. The
    /// WireGuard key and IPs are local placeholders: the Warren Quinn tunnel
    /// authenticates via the Ed25519 wallet seed, never these values.
    public static func walletBacked(ss58Address: String, publicKeyHex: String) -> DeviceState {
        let now = Date()
        let account = StoredAccountData(
            identifier: publicKeyHex,
            number: ss58Address,
            expiry: .distantFuture
        )
        let placeholderKey = WireGuard.PrivateKey(rawValue: Data(repeating: 0, count: 32))!
        let device = StoredDeviceData(
            creationDate: now,
            identifier: publicKeyHex,
            name: "warren-wallet",
            hijackDNS: false,
            ipv4Address: IPAddressRange(from: "10.64.0.1/32")!,
            ipv6Address: IPAddressRange(from: "fc00::1/128")!,
            wgKeyData: StoredWgKeyData(creationDate: now, privateKey: placeholderKey)
        )
        return .loggedIn(account, device)
    }

    private enum LoggedInCodableKeys: String, CodingKey {
        case _0 = "account"
        case _1 = "device"
    }

    public var isLoggedIn: Bool {
        switch self {
        case .loggedIn:
            return true
        case .loggedOut, .revoked:
            return false
        }
    }

    public var accountData: StoredAccountData? {
        switch self {
        case let .loggedIn(accountData, _):
            return accountData
        case .loggedOut, .revoked:
            return nil
        }
    }

    public var deviceData: StoredDeviceData? {
        switch self {
        case let .loggedIn(_, deviceData):
            return deviceData
        case .loggedOut, .revoked:
            return nil
        }
    }
}
