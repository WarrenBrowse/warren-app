//
//  WarrenDeviceRegistryClient.swift
//  WarrenRustRuntime
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  The account's devices (warren-core doc 115) as the app lists, renames and
//  revokes them. The calls are signed and sent in Rust
//  (`warren_devices_ffi`); this facade decodes the envelopes the shared
//  `warren_standing::device_registry` defines. The list names the account's
//  installs and the days they were used: it belongs on the account's own
//  screens and never in a log.
//

import Foundation

/// A UTC day, the only activity precision the registry keeps.
public struct WarrenRegistryDay: Comparable, Hashable, Sendable {
    public let year: Int
    public let month: Int
    public let day: Int

    /// The registry's `YYYY-MM-DD`; `nil` for anything else.
    public init?(_ string: String) {
        let parts = string.split(separator: "-", omittingEmptySubsequences: false)
        guard parts.count == 3, parts[0].count == 4, parts[1].count == 2, parts[2].count == 2,
            let year = Int(parts[0]), let month = Int(parts[1]), let day = Int(parts[2]),
            (1...12).contains(month), (1...31).contains(day)
        else {
            return nil
        }
        self.year = year
        self.month = month
        self.day = day
    }

    /// The UTC day `date` falls on, the day the registry counts it in.
    public init(utc date: Date) {
        let components = Self.utcCalendar.dateComponents([.year, .month, .day], from: date)
        year = components.year ?? 1970
        month = components.month ?? 1
        day = components.day ?? 1
    }

    /// Midnight UTC of the day, for a formatter set to UTC.
    public var date: Date {
        Self.utcCalendar.date(from: DateComponents(year: year, month: month, day: day)) ?? Date(timeIntervalSince1970: 0)
    }

    public static func < (lhs: Self, rhs: Self) -> Bool {
        (lhs.year, lhs.month, lhs.day) < (rhs.year, rhs.month, rhs.day)
    }

    private static let utcCalendar: Calendar = {
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = TimeZone(identifier: "UTC")!
        return calendar
    }()
}

/// One device of the logged-in account, as the registry lists it.
public struct WarrenAccountDevice: Equatable, Sendable, Identifiable {
    public let id: String
    /// As the registry keeps it: another device of the account chose it.
    public let name: String
    /// The registry's spelling: `android`, `ios`, `macos`, `windows`,
    /// `linux`, `cli`, `browser`.
    public let platform: String
    public let appVersion: String
    /// `nil` when the registry wrote a day this client cannot read.
    public let lastSeenOn: WarrenRegistryDay?
    public let thisDevice: Bool

    public init(
        id: String,
        name: String,
        platform: String,
        appVersion: String,
        lastSeenOn: WarrenRegistryDay?,
        thisDevice: Bool
    ) {
        self.id = id
        self.name = name
        self.platform = platform
        self.appVersion = appVersion
        self.lastSeenOn = lastSeenOn
        self.thisDevice = thisDevice
    }
}

public struct WarrenAccountDeviceList: Equatable, Sendable {
    public let devices: [WarrenAccountDevice]
    /// The last day a device of this account took the place of a Warren
    /// client too old to register; `nil` when that never happened.
    public let olderClientSeenOn: WarrenRegistryDay?

    public init(devices: [WarrenAccountDevice], olderClientSeenOn: WarrenRegistryDay?) {
        self.devices = devices
        self.olderClientSeenOn = olderClientSeenOn
    }
}

/// Why a registry call did nothing. The raw values are the envelope's codes.
public enum WarrenDeviceRegistryError: String, Error, Equatable, Sendable {
    /// No wallet is on the device.
    case noWallet = "no_wallet"
    /// A malformed device id.
    case invalidDevice = "invalid_device"
    /// A name the registry refuses.
    case invalidName = "invalid_name"
    /// The account has no such active device.
    case notFound = "not_found"
    /// This device was revoked from another device of the account.
    case revoked
    /// Nothing happened; a retry can work.
    case unavailable

    /// An unknown code reads as `unavailable`: a newer crate adding one must
    /// not read as a success.
    public init(code: String?) {
        self = code.flatMap(Self.init(rawValue:)) ?? .unavailable
    }
}

/// The registry calls, as Swift sees them. Each blocks on the network: call
/// off the main thread. The seed is never logged on either side.
public enum WarrenDeviceRegistryClient {
    private static let seedByteCount = 32

    /// The account's devices, this one marked.
    public static func list(seed: Data) -> Result<WarrenAccountDeviceList, WarrenDeviceRegistryError> {
        list(fromEnvelope: call(seed: seed) { warren_device_list($0) })
    }

    /// Renames the account's device `id`; answers whether it is this device.
    public static func rename(seed: Data, id: String, name: String) -> Result<Bool, WarrenDeviceRegistryError> {
        action(fromEnvelope: call(seed: seed) { seedPointer in
            id.withCString { idC in
                name.withCString { nameC in warren_device_rename(seedPointer, idC, nameC) }
            }
        })
    }

    /// Revokes the account's device `id`; answers whether it is this device,
    /// which the process then reads as revoked.
    public static func revoke(seed: Data, id: String) -> Result<Bool, WarrenDeviceRegistryError> {
        action(fromEnvelope: call(seed: seed) { seedPointer in
            id.withCString { idC in warren_device_revoke(seedPointer, idC) }
        })
    }

    /// Whether a registry call of this process learned this device's
    /// revocation since `forgetRevocation()`.
    public static var revokedInThisProcess: Bool {
        warren_device_revoked()
    }

    /// The revocation was taken in: the wallet left, or the user logged in
    /// again.
    public static func forgetRevocation() {
        warren_device_forget_revocation()
    }

    private static func call(
        seed: Data,
        _ body: (UnsafePointer<UInt8>) -> UnsafeMutablePointer<CChar>?
    ) -> String? {
        guard seed.count == seedByteCount else {
            return #"{"ok":false,"error":"no_wallet"}"#
        }
        let raw = seed.withUnsafeBytes { buffer -> UnsafeMutablePointer<CChar>? in
            guard let base = buffer.bindMemory(to: UInt8.self).baseAddress else { return nil }
            return body(base)
        }
        guard let raw else { return nil }
        defer { warren_wallet_free_mnemonic(raw) }
        return String(cString: raw)
    }

    // MARK: - Envelope decoding (pure, tested off-device)

    /// `{"ok":true,"devices":[..],"older_client_seen_on":"YYYY-MM-DD"|null}`
    /// or the classed failure. A row without an id is dropped: nothing could
    /// act on it.
    static func list(fromEnvelope envelope: String?) -> Result<WarrenAccountDeviceList, WarrenDeviceRegistryError> {
        okObject(envelope).map { object in
            WarrenAccountDeviceList(
                devices: (object["devices"] as? [[String: Any]] ?? []).compactMap(device(from:)),
                olderClientSeenOn: (object["older_client_seen_on"] as? String).flatMap(WarrenRegistryDay.init)
            )
        }
    }

    /// `{"ok":true,"this_device":bool}` or the classed failure.
    static func action(fromEnvelope envelope: String?) -> Result<Bool, WarrenDeviceRegistryError> {
        okObject(envelope).map { $0["this_device"] as? Bool == true }
    }

    private static func okObject(_ envelope: String?) -> Result<[String: Any], WarrenDeviceRegistryError> {
        // The decoder's message is never surfaced: it would quote the input,
        // which names the account's devices.
        guard let envelope, let data = envelope.data(using: .utf8),
            let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
        else {
            return .failure(.unavailable)
        }
        guard object["ok"] as? Bool == true else {
            return .failure(WarrenDeviceRegistryError(code: object["error"] as? String))
        }
        return .success(object)
    }

    private static func device(from row: [String: Any]) -> WarrenAccountDevice? {
        guard let id = row["device_id"] as? String, !id.isEmpty else { return nil }
        return WarrenAccountDevice(
            id: id,
            name: row["name"] as? String ?? "",
            platform: row["platform"] as? String ?? "",
            appVersion: row["app_version"] as? String ?? "",
            lastSeenOn: (row["last_seen_on"] as? String).flatMap(WarrenRegistryDay.init),
            thisDevice: row["this_device"] as? Bool == true
        )
    }
}
