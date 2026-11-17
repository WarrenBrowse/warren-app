//
//  PersistentAccessMethod.swift
//  MullvadVPN
//
//  Created by pronebird on 15/11/2023.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import Foundation
import Network

/// Persistent access method container model.
public struct PersistentAccessMethodStore: Codable {
    /// The last successfully reached access method.
    public var lastReachableAccessMethod: PersistentAccessMethod

    /// Persistent access method models.
    public var accessMethods: [PersistentAccessMethod]

    public init(lastReachableAccessMethod: PersistentAccessMethod, accessMethods: [PersistentAccessMethod]) {
        self.lastReachableAccessMethod = lastReachableAccessMethod
        self.accessMethods = accessMethods
    }

    /// Reads a store written before the bridges, encrypted DNS proxy and domain fronting methods were
    /// removed: their entries are dropped, and a last reachable method among them falls back to the
    /// direct method, so the user's own methods survive the upgrade.
    public init(from decoder: any Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        accessMethods = try container.decode([StoredAccessMethod].self, forKey: .accessMethods)
            .compactMap(\.method)

        let lastReachable = try container.decode(StoredAccessMethod.self, forKey: .lastReachableAccessMethod)
        if let method = lastReachable.method {
            lastReachableAccessMethod = method
        } else if let direct = accessMethods.first(where: { $0.proxyConfiguration == .direct }) {
            lastReachableAccessMethod = direct
        } else {
            throw DecodingError.dataCorruptedError(
                forKey: .lastReachableAccessMethod,
                in: container,
                debugDescription: "No direct access method to fall back to"
            )
        }
    }
}

/// An access method entry as stored, `nil` when it names one of the removed proxy configurations.
private struct StoredAccessMethod: Decodable {
    private enum CodingKeys: String, CodingKey {
        case proxyConfiguration
    }

    private enum RemovedProxyConfiguration: String, CodingKey {
        case bridges, encryptedDNS, domainFronting
    }

    let method: PersistentAccessMethod?

    init(from decoder: any Decoder) throws {
        do {
            method = try PersistentAccessMethod(from: decoder)
        } catch {
            let configuration = try decoder.container(keyedBy: CodingKeys.self)
                .nestedContainer(keyedBy: RemovedProxyConfiguration.self, forKey: .proxyConfiguration)
            guard !configuration.allKeys.isEmpty else { throw error }
            method = nil
        }
    }
}

/// Persistent access method model.
public struct PersistentAccessMethod: Identifiable, Codable, Equatable, Sendable, CustomDebugStringConvertible {
    /// The unique identifier used for referencing the access method entry in a persistent store.
    public var id: UUID

    /// The user-defined name for access method.
    public var name: String

    /// The flag indicating whether configuration is enabled.
    public var isEnabled: Bool

    /// Proxy configuration.
    public var proxyConfiguration: PersistentProxyConfiguration

    public var debugDescription: String {
        "Access Method Name: \(name) with: \(proxyConfiguration)"
    }

    public init(id: UUID, name: String, isEnabled: Bool, proxyConfiguration: PersistentProxyConfiguration) {
        self.id = id
        self.name = name
        self.isEnabled = isEnabled
        self.proxyConfiguration = proxyConfiguration
    }

    public init(from decoder: any Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)

        self.id = try container.decode(UUID.self, forKey: .id)
        self.isEnabled = try container.decode(Bool.self, forKey: .isEnabled)
        self.proxyConfiguration = try container.decode(PersistentProxyConfiguration.self, forKey: .proxyConfiguration)

        // Added after release of API access methods feature. There was previously no limitation on text input length,
        // so this formatting has been added to prevent already stored names from being too long when displayed.
        let name = try container.decode(String.self, forKey: .name)
        self.name = NameInputFormatter.format(name)
    }

    public static func == (lhs: Self, rhs: Self) -> Bool {
        lhs.id == rhs.id
    }
}
