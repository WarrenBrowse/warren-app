//
//  WarrenVersionCheck.swift
//  WarrenRustRuntime
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  Swift facade over the `warren_version_ffi` Rust exports
//  (`warren-ios/src/warren_version_ffi.rs`). The Rust side verifies the
//  Ed25519 signature of the fetched update manifest against the pinned
//  trusted metadata key (the same `mullvad-update` verifier the desktop
//  daemon and Android use) and applies the shared
//  `minimum_supported_version` forced-update rule. No crypto in Swift.
//

import Foundation
import WarrenRustRuntimeProxy

/// Outcome of verifying a signed update manifest against the running
/// app version.
public struct WarrenVerifiedVersionInfo: Codable, Equatable, Sendable {
    /// Whether the running version is still allowed to run. `false`
    /// means the forced-update gate must engage.
    public let supported: Bool
    /// Highest version listed in the manifest, if any.
    public let latestVersion: String?

    enum CodingKeys: String, CodingKey {
        case supported
        case latestVersion = "latest_version"
    }

    public init(supported: Bool, latestVersion: String?) {
        self.supported = supported
        self.latestVersion = latestVersion
    }
}

public enum WarrenVersionManifestVerifier {
    /// Verifies `manifest` (raw bytes of the fetched `ios.json`) and
    /// evaluates it against `currentVersion`.
    ///
    /// Returns `nil` when verification fails (bad signature, expired
    /// metadata, unparseable manifest): callers must then treat the
    /// manifest as absent and never trust its content.
    public static func verify(manifest: Data, currentVersion: String) -> WarrenVerifiedVersionInfo? {
        let rawResult: UnsafeMutablePointer<CChar>? = manifest.withUnsafeBytes { buffer in
            // An empty manifest can carry a null base address; feed the FFI a
            // dangling-but-unread pointer with length 0 instead of crashing.
            guard let base = buffer.baseAddress else {
                return nil
            }
            return warren_version_check_verify(
                base.assumingMemoryBound(to: UInt8.self),
                UInt(buffer.count),
                currentVersion
            )
        }
        guard let rawResult else { return nil }
        defer { warren_version_check_free(rawResult) }

        let json = Data(String(cString: rawResult).utf8)
        return try? JSONDecoder().decode(WarrenVerifiedVersionInfo.self, from: json)
    }
}
