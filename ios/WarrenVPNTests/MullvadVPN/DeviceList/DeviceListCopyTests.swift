//
//  DeviceListCopyTests.swift
//  WarrenVPNTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import Foundation
import XCTest

@testable import WarrenVPN

/// The device list's copy (warren-core doc 115 section 7.1), held to its
/// string catalog: every string in every language the app ships, with its
/// placeholders, and no em-dash or en-dash in any of them.
final class DeviceListCopyTests: XCTestCase {
    /// `ios/WarrenVPNTests/MullvadVPN/DeviceList/DeviceListCopyTests.swift`
    private static let iosDir = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .deletingLastPathComponent()

    static let keys = [
        "Devices",
        "Devices signed in to this account. Disconnecting one signs it out: it needs the recovery phrase to sign in again.",
        "This device",
        "Active today",
        "Last active %@",
        "Last seen %@",
        "Command line",
        "Browser extension",
        "Other platform",
        "Rename device",
        "Disconnect %@?",
        "It will be signed out within 10 minutes and will need the recovery phrase to sign in again. For a lost or stolen device, move to a new account instead: a device that keeps the recovery phrase can sign in again.",
        "Could not load your devices. Try again.",
        "Could not disconnect the device. Try again.",
        "Could not rename the device. Try again.",
        "An older version of Warren",
        "A device of this account runs a version of Warren older than 1.1.36. It no longer blocks your other devices. Update or uninstall it.",
        "Something went wrong while logging out.",
        DeviceListCopy.disconnectKey,
    ]

    func testEveryDeviceListStringIsLocalizedInEveryLanguageWithItsPlaceholders() throws {
        let catalog = try loadCatalog()
        let languages = try XCTUnwrap(catalog["Manage devices"]).keys.filter { $0 != "en" }.sorted()
        XCTAssertEqual(languages.count, 23, "the reference key lost languages")

        var failures: [String] = []
        for key in Self.keys {
            guard let localizations = catalog[key] else {
                failures.append("absent from the catalog: \(key.prefix(50))")
                continue
            }
            let placeholders = key.components(separatedBy: "%@").count - 1
            for language in languages {
                let value = localizations[language] ?? ""
                if value.isEmpty {
                    failures.append("\(language): untranslated \(key.prefix(50))")
                } else if value.components(separatedBy: "%@").count - 1 != placeholders {
                    failures.append("\(language): placeholders differ in \(key.prefix(50))")
                }
            }
        }
        XCTAssertEqual(failures, [], "\(failures.count) unlocalized device list strings")
    }

    func testNoDeviceListStringCarriesAnEmDashOrAnEnDash() throws {
        let catalog = try loadCatalog()
        var failures: [String] = []
        for key in Self.keys {
            for (language, value) in catalog[key] ?? [:] where value.contains("\u{2014}") || value.contains("\u{2013}") {
                failures.append("\(language): \(key.prefix(50))")
            }
        }
        XCTAssertEqual(failures, [])
    }

    /// The row's "Disconnect" has its own key: the VPN's "Disconnect" is a
    /// noun or a different verb in several languages.
    func testTheDeviceDisconnectActionReadsDisconnectInEnglish() {
        XCTAssertEqual(DeviceListCopy.disconnect, "Disconnect")
    }

    /// The catalog as key to (language to value).
    private func loadCatalog() throws -> [String: [String: String]] {
        let data = try Data(contentsOf: Self.iosDir.appendingPathComponent("Assets/Localizable.xcstrings"))
        let root = try XCTUnwrap(try JSONSerialization.jsonObject(with: data) as? [String: Any])
        let strings = try XCTUnwrap(root["strings"] as? [String: Any])
        return strings.reduce(into: [:]) { catalog, entry in
            guard let body = entry.value as? [String: Any],
                let localizations = body["localizations"] as? [String: Any]
            else { return }
            catalog[entry.key] = localizations.reduce(into: [:]) { values, localization in
                guard let unit = (localization.value as? [String: Any])?["stringUnit"] as? [String: Any],
                    let value = unit["value"] as? String
                else { return }
                values[localization.key] = value
            }
        }
    }
}
