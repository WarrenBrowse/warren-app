//
//  WarrenThemeTests.swift
//  WarrenVPNTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import UIKit
import WarrenSettings
import XCTest

@testable import WarrenVPN

/// The theme choice the connect screen is painted with, on the same rule as
/// desktop's `resolveTheme` (`src/shared/theme.ts`): follow the device by
/// default, dark when the device states nothing, and an explicit choice wins.
final class WarrenThemeTests: XCTestCase {
    private static let iosDir = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .deletingLastPathComponent()

    private var storedPreference: Any?

    override func setUp() {
        super.setUp()
        storedPreference = UserDefaults.standard.object(forKey: "warrenThemePreference")
    }

    override func tearDown() {
        UserDefaults.standard.set(storedPreference, forKey: "warrenThemePreference")
        super.tearDown()
    }

    func testSystemFollowsTheDeviceAppearance() {
        XCTAssertEqual(WarrenTheme(preference: .system, systemStyle: .light), .light)
        XCTAssertEqual(WarrenTheme(preference: .system, systemStyle: .dark), .dark)
    }

    func testSystemPaintsDarkWhenTheDeviceSaysNothing() {
        XCTAssertEqual(WarrenTheme(preference: .system, systemStyle: .unspecified), .dark)
    }

    func testAnExplicitChoiceWinsOverTheDevice() {
        XCTAssertEqual(WarrenTheme(preference: .light, systemStyle: .dark), .light)
        XCTAssertEqual(WarrenTheme(preference: .dark, systemStyle: .light), .dark)
    }

    func testTheResolvedThemeNamesItsInterfaceStyle() {
        XCTAssertEqual(WarrenTheme.dark.userInterfaceStyle, .dark)
        XCTAssertEqual(WarrenTheme.light.userInterfaceStyle, .light)
    }

    func testANewInstallFollowsTheSystem() {
        UserDefaults.standard.removeObject(forKey: "warrenThemePreference")

        XCTAssertEqual(AppPreferences().warrenThemePreference, .system)
    }

    /// The stored value may come from a later version: anything this one does
    /// not know follows the system rather than failing.
    func testAStoredValueThisVersionDoesNotKnowFollowsTheSystem() {
        UserDefaults.standard.set("sepia", forKey: "warrenThemePreference")

        XCTAssertEqual(AppPreferences().warrenThemePreference, .system)
    }

    func testTheChoiceSurvivesARelaunch() {
        let writer = AppPreferences()
        writer.warrenThemePreference = .light

        XCTAssertEqual(AppPreferences().warrenThemePreference, .light)

        writer.warrenThemePreference = .dark
        XCTAssertEqual(AppPreferences().warrenThemePreference, .dark)
    }

    /// The three choices and the row that holds them are read in every
    /// language the app ships, with desktop's words (`user-interface-settings-view`).
    func testTheSettingIsLocalizedInEveryLanguage() throws {
        let data = try Data(contentsOf: Self.iosDir.appendingPathComponent("Assets/Settings.xcstrings"))
        let root = try XCTUnwrap(try JSONSerialization.jsonObject(with: data) as? [String: Any])
        let strings = try XCTUnwrap(root["strings"] as? [String: Any])
        let reference = try XCTUnwrap(
            (strings["Warren beta"] as? [String: Any])?["localizations"] as? [String: Any]
        )
        let languages = Set(reference.keys).subtracting(["en"])
        XCTAssertEqual(languages.count, 23, "the reference key lost languages")

        let keys = ["Theme"] + WarrenThemePreference.allCases.map(\.localizationKey)
        XCTAssertEqual(keys, ["Theme", "System", "Dark", "Light"])
        var failures: [String] = []
        for key in keys {
            let localizations = (strings[key] as? [String: Any])?["localizations"] as? [String: Any] ?? [:]
            for language in languages.sorted() {
                let unit = (localizations[language] as? [String: Any])?["stringUnit"] as? [String: Any]
                let value = unit?["value"] as? String ?? ""
                if value.isEmpty {
                    failures.append("\(language): untranslated \(key)")
                } else if value.contains("\u{2014}") || value.contains("\u{2013}") {
                    failures.append("\(language): dash in \(key)")
                }
            }
        }
        XCTAssertEqual(failures, [])
    }
}
