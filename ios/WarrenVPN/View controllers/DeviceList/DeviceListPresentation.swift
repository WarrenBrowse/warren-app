//
//  DeviceListPresentation.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import Foundation
import WarrenRustRuntime

/// How the account's devices read on screen (warren-core doc 115 section
/// 7.1), the same rules the desktop and Android lists apply.
enum DeviceListPresentation {
    /// The longest name the registry keeps, in characters.
    static let nameMaxCharacters = 40

    /// Explicit embeddings, overrides and isolates (U+202A to U+202E, U+2066
    /// to U+2069): a name another device chose could otherwise reorder the
    /// text around it.
    static func displayedName(_ name: String) -> String {
        String(String.UnicodeScalarView(name.unicodeScalars.filter { !isBidiControl($0) }))
    }

    /// Whether the registry keeps this name: 1 to 40 characters (code points,
    /// as the registry counts them) once trimmed, and no control character.
    static func isValidName(_ name: String) -> Bool {
        let scalars = name.trimmingCharacters(in: .whitespacesAndNewlines).unicodeScalars
        return (1...nameMaxCharacters).contains(scalars.count)
            && !scalars.contains { $0.properties.generalCategory == .control }
    }

    /// This device first, then the others by last activity, the most recent
    /// first, then by name.
    static func inDisplayOrder(_ devices: [WarrenAccountDevice]) -> [WarrenAccountDevice] {
        devices.sorted { lhs, rhs in
            if lhs.thisDevice != rhs.thisDevice {
                return lhs.thisDevice
            }
            if lhs.lastSeenOn != rhs.lastSeenOn {
                guard let left = lhs.lastSeenOn else { return false }
                guard let right = rhs.lastSeenOn else { return true }
                return left > right
            }
            return displayedName(lhs.name) < displayedName(rhs.name)
        }
    }

    /// The platform as the list names it. Product names stay as they are.
    static func platformLabel(_ platform: String) -> String {
        switch platform {
        case "android": "Android"
        case "ios": "iOS"
        case "macos": "macOS"
        case "windows": "Windows"
        case "linux": "Linux"
        case "cli": NSLocalizedString("Command line", comment: "Platform of a device of the account: the Warren command-line client.")
        case "browser": NSLocalizedString("Browser extension", comment: "Platform of a device of the account: the Warren browser extension.")
        default: NSLocalizedString("Other platform", comment: "Platform of a device of the account that this version does not know.")
        }
    }

    /// "<Platform> · <version>", the second line of a device row.
    static func platformLine(_ device: WarrenAccountDevice) -> String {
        "\(platformLabel(device.platform)) · \(device.appVersion)"
    }

    /// "Active today", or the last day the device was seen. The registry
    /// counts days in UTC, so "today" is the UTC day.
    static func activityLabel(lastSeenOn: WarrenRegistryDay, today: WarrenRegistryDay, locale: Locale) -> String {
        if lastSeenOn == today {
            return NSLocalizedString("Active today", comment: "Shown under a device of the account that was used today.")
        }
        return String(
            format: NSLocalizedString(
                "Last active %@",
                comment: "Shown under a device of the account: the last day it was used, in the short form of the language."
            ),
            shortDate(lastSeenOn, locale: locale)
        )
    }

    /// The day an older Warren client was last seen, for the older-client row.
    static func olderClientSeenLabel(_ day: WarrenRegistryDay, locale: Locale) -> String {
        String(
            format: NSLocalizedString(
                "Last seen %@",
                comment: "Shown on the row of an older Warren client of the account: a day in the short form of the language."
            ),
            shortDate(day, locale: locale)
        )
    }

    /// A registry day in the locale's short form.
    static func shortDate(_ day: WarrenRegistryDay, locale: Locale) -> String {
        let formatter = DateFormatter()
        formatter.locale = locale
        formatter.timeZone = TimeZone(identifier: "UTC")
        formatter.dateStyle = .short
        formatter.timeStyle = .none
        return formatter.string(from: day.date)
    }

    private static func isBidiControl(_ scalar: Unicode.Scalar) -> Bool {
        (0x202A...0x202E).contains(scalar.value) || (0x2066...0x2069).contains(scalar.value)
    }
}

/// Copy whose English is shared with another meaning elsewhere in the app.
enum DeviceListCopy {
    /// The row action and the confirmation button. The VPN's "Disconnect"
    /// is a noun or another verb in several languages, so this one carries
    /// its own key, translated as the desktop's device-management context.
    static let disconnectKey = "device-management.disconnect"

    static var disconnect: String {
        NSLocalizedString(disconnectKey, comment: "Disconnects another device of the account.")
    }
}
