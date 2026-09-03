//
//  WarrenProductAnchors.swift
//  WarrenRustRuntime
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  The compiled product environment's anchor table, read once from the Rust
//  `warren-product-env` crate through `warren_product_anchors()`. Swift reads
//  the deep-link scheme and the hosts here instead of spelling them again:
//  the crate is the reference, and the iOS build compiles it for the same
//  `WARREN_PRODUCT_ENV` the Xcode configuration registers in Info.plist.
//  The columns are those of `fixtures/client-rules/product_env.json`.
//

import Foundation
import WarrenRustRuntimeProxy

/// One row of the product table: every anchor that differs between the prod,
/// staging and beta builds, plus the two a single broker and forum serve.
public struct WarrenProductAnchors: Equatable, Sendable {
    /// `prod`, `staging` or `beta`.
    public let name: String
    public let apiURL: String
    public let apiHost: String
    public let displayName: String
    public let applicationID: String
    /// The URL scheme this build answers deep links on (`warren`,
    /// `warren-beta`, `warren-staging`).
    public let deepLinkScheme: String
    /// The one host a forum deep link may name and every forum request goes to.
    public let connectHost: String
    /// The forum origin, a bare https URL.
    public let forumPublicURL: String

    /// The table of the environment this binary was compiled for.
    public static let current: WarrenProductAnchors = load()

    /// The tables of the environments that OUTRANK this build, strongest
    /// first (prod, then staging), empty on prod. The one thing [`current`]
    /// cannot answer: coexistence needs another install's URL scheme to look
    /// for it, and the compiled row only ever names this build.
    ///
    /// FAIL-SAFE DIRECTION: a list that cannot be read is EMPTY, so this
    /// build keeps running. Wrongly yielding disarms its own kill switch on
    /// no evidence, while wrongly staying up only leaves two idle installs.
    /// That is the direction `environments_with_priority_over` documents in
    /// the Rust crate, and the opposite of the orphan-firewall sweep.
    public static let higherPriority: [WarrenProductAnchors] = loadHigherPriority()

    /// The shipped production build. Prod carries no marker anywhere and none
    /// of its strings may move: a rename reaches every existing install.
    public var isProd: Bool { name == "prod" }

    /// The short marker a non-prod build wears beside the wordmark and in the
    /// name iOS gives its VPN configuration, `nil` on prod. Derived from the
    /// environment name so a new row needs no second table to be added to.
    public var environmentBadge: String? { isProd ? nil : name.uppercased() }

    /// Decodes the JSON object the FFI returns. `nil` when a column is
    /// missing, which the crate's own tests rule out for the live table; the
    /// unit test decodes the fixture rows through the same path.
    static func decode(_ json: String) -> WarrenProductAnchors? {
        guard let data = json.data(using: .utf8),
            let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
        else {
            return nil
        }
        return decode(object)
    }

    /// Decodes the JSON array of rows the higher-priority FFI returns. `nil`
    /// when the array, or any row in it, has an unexpected shape: a partial
    /// list would silently drop an environment this build must watch.
    static func decodeAll(_ json: String) -> [WarrenProductAnchors]? {
        guard let data = json.data(using: .utf8),
            let rows = try? JSONSerialization.jsonObject(with: data) as? [[String: Any]]
        else {
            return nil
        }
        var decoded: [WarrenProductAnchors] = []
        for row in rows {
            guard let anchors = decode(row) else { return nil }
            decoded.append(anchors)
        }
        return decoded
    }

    private static func decode(_ object: [String: Any]) -> WarrenProductAnchors? {
        guard let name = object["name"] as? String,
            let apiURL = object["api_url"] as? String,
            let apiHost = object["api_host"] as? String,
            let displayName = object["display_name"] as? String,
            let applicationID = object["application_id"] as? String,
            let deepLinkScheme = object["deep_link_scheme"] as? String,
            let connectHost = object["connect_host"] as? String,
            let forumPublicURL = object["forum_public_url"] as? String
        else {
            return nil
        }
        return WarrenProductAnchors(
            name: name,
            apiURL: apiURL,
            apiHost: apiHost,
            displayName: displayName,
            applicationID: applicationID,
            deepLinkScheme: deepLinkScheme,
            connectHost: connectHost,
            forumPublicURL: forumPublicURL
        )
    }

    private static func load() -> WarrenProductAnchors {
        guard let raw = warren_product_anchors() else {
            fatalError("warren_product_anchors: the Rust product table could not be rendered")
        }
        defer { warren_product_anchors_free(raw) }
        guard let anchors = decode(String(cString: raw)) else {
            fatalError("warren_product_anchors: the Rust product table has an unexpected shape")
        }
        return anchors
    }

    private static func loadHigherPriority() -> [WarrenProductAnchors] {
        guard let raw = warren_higher_priority_product_anchors() else { return [] }
        defer { warren_product_anchors_free(raw) }
        return decodeAll(String(cString: raw)) ?? []
    }
}
