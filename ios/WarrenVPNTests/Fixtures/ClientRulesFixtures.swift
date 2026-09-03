//
//  ClientRulesFixtures.swift
//  WarrenVPNTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import Foundation

/// The cross-platform client-rule fixtures (`fixtures/client-rules/README.md`
/// at the repository root), read here the way the Rust crates, the JVM suite
/// and the desktop suite read them: one file, several readers, no copy.
/// Located from this source file's own path; the simulator process runs on
/// the host and reads the checkout directly, so the test bundle carries no
/// copy that could go stale.
enum ClientRulesFixtures {
    enum Failure: Error {
        case notAnObject(String)
        case missingKey(String)
    }

    static let directory: URL = {
        // ios/WarrenVPNTests/Fixtures/ClientRulesFixtures.swift
        URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("fixtures/client-rules", isDirectory: true)
    }()

    static func load(_ name: String) throws -> [String: Any] {
        let data = try Data(contentsOf: directory.appendingPathComponent(name))
        guard let object = try JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            throw Failure.notAnObject(name)
        }
        return object
    }

    /// Whether the iOS reader must leave `case` alone (a divergence a later lot closes).
    static func skippedOnIOS(_ testCase: [String: Any]) -> Bool {
        (testCase["skip"] as? [String])?.contains("ios") == true
    }

    static func cases(_ object: [String: Any], _ key: String) throws -> [[String: Any]] {
        guard let cases = object[key] as? [[String: Any]] else { throw Failure.missingKey(key) }
        return cases
    }

    static func string(_ object: [String: Any], _ key: String) throws -> String {
        guard let value = object[key] as? String else { throw Failure.missingKey(key) }
        return value
    }

    static func object(_ object: [String: Any], _ key: String) throws -> [String: Any] {
        guard let value = object[key] as? [String: Any] else { throw Failure.missingKey(key) }
        return value
    }
}
