//
//  WarrenForumLoginCopyTests.swift
//  WarrenVPNTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import Foundation
import XCTest

/// The consent prompt of the forum sign-in flow, held to its string catalog.
///
/// The prompt is the only place the relayed-approval attack is explained, so a
/// locale that falls back to English there is a locale where the warning is not
/// read: half of this flow shipped localized in 24 languages and half did not.
/// This reads the keys straight out of `WarrenForumLogin.swift`, so a string
/// added to the flow and never localized fails here rather than at a user.
///
/// The second property is that the cross-device copy names no origin: it is
/// raised both by a deep link carrying `xd=1` (the QR on the approval page) and
/// by a code typed under Settings, which has no QR anywhere in its flow.
final class WarrenForumLoginCopyTests: XCTestCase {
    /// `ios/WarrenVPNTests/MullvadVPN/Forum/WarrenForumLoginCopyTests.swift`
    private static let iosDir = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .deletingLastPathComponent()

    /// "QR" survives untranslated in most locales; Simplified Chinese and Thai
    /// spell it out, so both spellings are listed rather than assuming the
    /// Latin token covers every language.
    private let qrTokens = ["QR", "二维码", "二維碼", "คิวอาร์"]

    func testEveryPromptStringIsLocalizedInEveryLanguage() throws {
        let catalog = try loadCatalog()
        let reference = try XCTUnwrap(catalog["This sign-in request has expired. Start again from the browser page."])
        // The catalog's source language is English and the key IS the English
        // value, so an entry may legitimately carry no "en" localization.
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
        XCTAssertEqual(failures, [], "\(failures.count) unlocalized prompt strings")
    }

    func testTheCrossDevicePromptNeverNamesAQrCode() throws {
        let catalog = try loadCatalog()
        var failures: [String] = []
        for key in try flowStrings() where isCrossDeviceKey(key) {
            for (language, value) in catalog[key] ?? [:] {
                for token in qrTokens where value.uppercased().contains(token.uppercased()) {
                    failures.append("\(language): \(key.prefix(40)) names \(token)")
                }
            }
        }
        XCTAssertEqual(failures, [], "\(failures.count) cross-device strings claim a QR origin")
    }

    /// The prompt copy is chosen by `link.crossDevice`, and the cross-device
    /// arm is the one a typed code also lands on.
    private func isCrossDeviceKey(_ key: String) -> Bool {
        key.contains("on another device")
            || key.contains("Warren cannot tell which browser")
            || key.contains("If someone sent you this code")
    }

    /// Every `NSLocalizedString` key the flow emits, read off its own source.
    private func flowStrings() throws -> [String] {
        let source = try String(
            contentsOf: Self.iosDir.appendingPathComponent("WarrenVPN/Classes/WarrenForumLogin.swift"),
            encoding: .utf8)
        let pattern = try NSRegularExpression(pattern: #"NSLocalizedString\(\s*"([^"]*)""#)
        let range = NSRange(source.startIndex..<source.endIndex, in: source)
        let keys = pattern.matches(in: source, range: range).compactMap { match -> String? in
            guard let found = Range(match.range(at: 1), in: source) else { return nil }
            return String(source[found])
        }
        XCTAssertGreaterThanOrEqual(keys.count, 12, "only \(keys.count) prompt strings reached this reader")
        return keys
    }

    /// The catalog as key to (language to value), the shape both tests read.
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
