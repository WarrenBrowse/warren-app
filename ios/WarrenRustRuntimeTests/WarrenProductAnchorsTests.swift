import XCTest

@testable import WarrenRustRuntime

/// The product table Swift reads through the FFI is the Rust crate's row for
/// the compiled environment, and that row is the one
/// `fixtures/client-rules/product_env.json` pins for every platform. A
/// scheme or host that Swift would spell on its own has no place to drift to.
final class WarrenProductAnchorsTests: XCTestCase {
    func testTheLiveTableIsTheFixtureRowOfTheCompiledEnvironment() throws {
        let anchors = WarrenProductAnchors.current
        let fixture = try ClientRulesFixtures.load("product_env.json")
        let environments = try ClientRulesFixtures.object(fixture, "environments")
        let row = try ClientRulesFixtures.object(environments, anchors.name)
        XCTAssertEqual(anchors.apiURL, try ClientRulesFixtures.string(row, "api_url"))
        XCTAssertEqual(anchors.apiHost, try ClientRulesFixtures.string(row, "api_host"))
        XCTAssertEqual(anchors.displayName, try ClientRulesFixtures.string(row, "display_name"))
        XCTAssertEqual(anchors.applicationID, try ClientRulesFixtures.string(row, "application_id"))
        XCTAssertEqual(anchors.deepLinkScheme, try ClientRulesFixtures.string(row, "deep_link_scheme"))
        XCTAssertEqual(anchors.connectHost, try ClientRulesFixtures.string(row, "connect_host"))
        XCTAssertEqual(anchors.forumPublicURL, try ClientRulesFixtures.string(row, "forum_public_url"))
    }

    func testEveryFixtureRowDecodesThroughTheSamePath() throws {
        // The decoder reads the columns the crate renders; every row of the
        // fixture has them all, so a column renamed on one side fails here.
        let fixture = try ClientRulesFixtures.load("product_env.json")
        let environments = try ClientRulesFixtures.object(fixture, "environments")
        XCTAssertEqual(Set(environments.keys), ["prod", "staging", "beta"])
        for (name, row) in environments {
            let data = try JSONSerialization.data(withJSONObject: row)
            let decoded = WarrenProductAnchors.decode(String(decoding: data, as: UTF8.self))
            XCTAssertEqual(decoded?.name, name)
            XCTAssertEqual(decoded?.deepLinkScheme, (row as? [String: Any])?["deep_link_scheme"] as? String)
        }
    }

    /// The marker every non-prod surface reads (the header chip, the name iOS
    /// gives the VPN configuration) is derived from the environment name, so a
    /// row added to the table is marked without a second list to update. Prod
    /// carries none: its strings are the shipped ones and must not move.
    func testEveryNonProdRowCarriesAMarkerAndProdCarriesNone() throws {
        let fixture = try ClientRulesFixtures.load("product_env.json")
        let environments = try ClientRulesFixtures.object(fixture, "environments")
        var markers: Set<String> = []
        for (name, row) in environments {
            let data = try JSONSerialization.data(withJSONObject: row)
            let decoded = try XCTUnwrap(WarrenProductAnchors.decode(String(decoding: data, as: UTF8.self)))
            if name == "prod" {
                XCTAssertTrue(decoded.isProd)
                XCTAssertNil(decoded.environmentBadge)
            } else {
                XCTAssertFalse(decoded.isProd, name)
                let marker = try XCTUnwrap(decoded.environmentBadge, name)
                XCTAssertEqual(marker, name.uppercased(), name)
                XCTAssertTrue(markers.insert(marker).inserted, "\(name): marker is not its own")
            }
        }
    }

    func testATableMissingAColumnIsRefused() {
        XCTAssertNil(WarrenProductAnchors.decode(#"{"name":"prod"}"#))
        XCTAssertNil(WarrenProductAnchors.decode("not json"))
    }
}
