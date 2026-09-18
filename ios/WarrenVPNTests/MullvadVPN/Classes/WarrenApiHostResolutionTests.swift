//
//  WarrenApiHostResolutionTests.swift
//  WarrenVPNTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import WarrenRustRuntime
import XCTest

@testable import WarrenVPN

/// A component never hardcodes an API host: it resolves it from its product
/// environment. The failure this rule exists to prevent already happened once
/// on the exit fleet, where every node stayed on the production hostname while
/// the stack had moved to beta, and nothing could notice because both names
/// resolved to the same box.
///
/// iOS carried two of them. The packet tunnel fetched its multi-hop directory
/// from `https://api.warrenbrowse.com` written out, and the update gate asked
/// the same production host whether the build was too old to run, in every
/// environment.
final class WarrenApiHostResolutionTests: XCTestCase {
    /// The anchors are the selector. Whatever they say is what the app must ask.
    func testTheAnchorsCarryAnApiHostForThisBuild() {
        let apiURL = WarrenProductAnchors.current.apiURL
        XCTAssertTrue(apiURL.hasPrefix("https://"), "apiURL is \(apiURL)")
        XCTAssertFalse(apiURL.hasSuffix("/"), "a trailing slash would double up in every path")
    }

    /// Grep is the only way to catch a literal that nobody calls yet. The
    /// resolver's own arm lives in `WarrenProductAnchors`, which is generated
    /// from `fixtures/client-rules/product_env.json` and is not searched here.
    func testNoSwiftSourceWritesAnApiHostOutByHand() throws {
        let root = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()  // Classes
            .deletingLastPathComponent()  // MullvadVPN
            .deletingLastPathComponent()  // WarrenVPNTests
            .deletingLastPathComponent()  // ios
        let searched = ["WarrenVPN", "PacketTunnel", "PacketTunnelCore", "Shared"]
        let hosts = ["api.warrenbrowse.com", "api.beta.warrenbrowse.com", "api.staging.warrenbrowse.com"]

        var offenders: [String] = []
        for directory in searched {
            let base = root.appendingPathComponent(directory)
            guard
                let walker = FileManager.default.enumerator(
                    at: base, includingPropertiesForKeys: nil)
            else { continue }
            for case let url as URL in walker where url.pathExtension == "swift" {
                guard let text = try? String(contentsOf: url, encoding: .utf8) else { continue }
                for line in text.split(separator: "\n", omittingEmptySubsequences: false) {
                    // A host named in prose is how the rule gets explained.
                    let code = line.drop { $0 == " " }
                    if code.hasPrefix("//") || code.hasPrefix("///") || code.hasPrefix("*") {
                        continue
                    }
                    if hosts.contains(where: { line.contains($0) }) {
                        offenders.append("\(url.lastPathComponent): \(line.trimmingCharacters(in: .whitespaces))")
                    }
                }
            }
        }
        XCTAssertEqual(
            offenders, [],
            "resolve the host from WarrenProductAnchors.current.apiURL instead")
    }
}
