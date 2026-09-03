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

    /// Decodes the JSON object the FFI returns. `nil` when a column is
    /// missing, which the crate's own tests rule out for the live table; the
    /// unit test decodes the fixture rows through the same path.
    static func decode(_ json: String) -> WarrenProductAnchors? {
        guard let data = json.data(using: .utf8),
            let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
            let name = object["name"] as? String,
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
}
