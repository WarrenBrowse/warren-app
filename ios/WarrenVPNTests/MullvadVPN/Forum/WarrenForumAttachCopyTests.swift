//
//  WarrenForumAttachCopyTests.swift
//  WarrenVPNTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import Foundation
import XCTest

/// The attach-logs consent and its result messages, held to the string
/// catalog the way the login prompt is: every key the flow and its view emit
/// is localized in every language the app ships, so no locale reads the
/// consent in English while the rest of the app speaks its own language.
final class WarrenForumAttachCopyTests: XCTestCase {
    /// `ios/WarrenVPNTests/MullvadVPN/Forum/WarrenForumAttachCopyTests.swift`
    private static let iosDir = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .deletingLastPathComponent()

    private static let sources = [
        "WarrenVPN/Classes/WarrenForumAttach.swift",
        "WarrenVPN/View controllers/Settings/WarrenForumAttachConsentView.swift",
    ]

    func testEveryAttachStringIsLocalizedInEveryLanguage() throws {
        let catalog = try loadCatalog()
        let reference = try XCTUnwrap(catalog["This sign-in request has expired. Start again from the browser page."])
        let languages = reference.keys.filter { $0 != "en" }.sorted()
        XCTAssertEqual(languages.count, 23, "the reference key lost languages")

        var failures: [String] = []
        for key in try flowStrings() {
            guard let localizations = catalog[key] else {
                failures.append("absent from the catalog: \(key.prefix(50))")
                continue
            }
            if let english = localizations["en"], english != key {
                failures.append("en: \(key.prefix(50)) does not match its own key")
            }
            for language in languages where (localizations[language] ?? "").isEmpty {
                failures.append("\(language): untranslated \(key.prefix(50))")
            }
        }
        XCTAssertEqual(failures, [], "\(failures.count) unlocalized attach strings")
    }

    func testNoAttachStringCarriesADash() throws {
        // The shared typography rule, in every language of every attach key.
        let catalog = try loadCatalog()
        var failures: [String] = []
        for key in try flowStrings() {
            for (language, value) in catalog[key] ?? [:] where value.contains("\u{2014}") || value.contains("\u{2013}") {
                failures.append("\(language): \(key.prefix(40))")
            }
        }
        XCTAssertEqual(failures, [], "\(failures.count) attach strings carry a dash")
    }

    func testTheTopicMessageCarriesTheTopicNumberInEveryLanguage() throws {
        let catalog = try loadCatalog()
        let key = try XCTUnwrap(try flowStrings().first { $0.contains("(topic %lld)") })
        for (language, value) in try XCTUnwrap(catalog[key]) where !value.contains("%lld") {
            XCTFail("\(language): the topic message lost its number placeholder")
        }
    }

    /// Every `NSLocalizedString` key the flow and its view emit, read off
    /// their own source.
    private func flowStrings() throws -> [String] {
        var keys: [String] = []
        let pattern = try NSRegularExpression(pattern: #"NSLocalizedString\(\s*"([^"]*)""#)
        for source in Self.sources {
            let text = try String(contentsOf: Self.iosDir.appendingPathComponent(source), encoding: .utf8)
            let range = NSRange(text.startIndex..<text.endIndex, in: text)
            keys += pattern.matches(in: text, range: range).compactMap { match -> String? in
                guard let found = Range(match.range(at: 1), in: text) else { return nil }
                return String(text[found])
            }
        }
        XCTAssertGreaterThanOrEqual(keys.count, 18, "only \(keys.count) attach strings reached this reader")
        return keys
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
