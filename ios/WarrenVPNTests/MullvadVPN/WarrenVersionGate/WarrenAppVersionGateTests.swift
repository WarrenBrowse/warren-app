//
//  WarrenAppVersionGateTests.swift
//  WarrenVPNTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import XCTest

@testable import WarrenRustRuntime
@testable import WarrenVPN

final class WarrenAppVersionGateTests: XCTestCase {
    private let day: TimeInterval = 24 * 60 * 60

    // MARK: - Check throttling

    func testCheckIsDueWhenNeverChecked() {
        XCTAssertTrue(warrenVersionCheckIsDue(lastCheck: nil, now: Date(), interval: day))
    }

    func testCheckIsNotDueWithinInterval() {
        let now = Date()
        XCTAssertFalse(
            warrenVersionCheckIsDue(lastCheck: now.addingTimeInterval(-60), now: now, interval: day)
        )
    }

    func testCheckIsDueAfterInterval() {
        let now = Date()
        XCTAssertTrue(
            warrenVersionCheckIsDue(
                lastCheck: now.addingTimeInterval(-day - 1),
                now: now,
                interval: day
            )
        )
    }

    func testCheckIsDueOnClockRollback() {
        let now = Date()
        XCTAssertTrue(
            warrenVersionCheckIsDue(lastCheck: now.addingTimeInterval(3600), now: now, interval: day)
        )
    }

    // MARK: - Gate decision

    func testGateFailsOpenWithoutSnapshot() {
        XCTAssertFalse(warrenUpdateGateShouldBlock(snapshot: nil, currentAppVersion: "2026.3"))
    }

    func testGateBlocksUnsupportedVersion() {
        let snapshot = WarrenVersionGateSnapshot(
            supported: false,
            latestVersion: "2026.9",
            checkedAppVersion: "2026.3"
        )
        XCTAssertTrue(warrenUpdateGateShouldBlock(snapshot: snapshot, currentAppVersion: "2026.3"))
    }

    func testGateAllowsSupportedVersion() {
        let snapshot = WarrenVersionGateSnapshot(
            supported: true,
            latestVersion: "2026.9",
            checkedAppVersion: "2026.3"
        )
        XCTAssertFalse(warrenUpdateGateShouldBlock(snapshot: snapshot, currentAppVersion: "2026.3"))
    }

    func testGateIgnoresSnapshotFromAnotherAppVersion() {
        // The app was updated since the blocking verdict was cached: fail
        // open until the next check re-evaluates the new version.
        let snapshot = WarrenVersionGateSnapshot(
            supported: false,
            latestVersion: "2026.9",
            checkedAppVersion: "2026.3"
        )
        XCTAssertFalse(warrenUpdateGateShouldBlock(snapshot: snapshot, currentAppVersion: "2026.9"))
    }

    // MARK: - Soft update-available path

    func testUpdateAvailableWhenManifestListsNewerVersion() {
        let snapshot = WarrenVersionGateSnapshot(
            supported: true,
            latestVersion: "2026.4",
            checkedAppVersion: "2026.3"
        )
        XCTAssertEqual(
            warrenUpdateAvailableVersion(snapshot: snapshot, currentAppVersion: "2026.3"),
            "2026.4"
        )
    }

    func testNoUpdateAvailableWhenUpToDate() {
        let snapshot = WarrenVersionGateSnapshot(
            supported: true,
            latestVersion: "2026.3",
            checkedAppVersion: "2026.3"
        )
        XCTAssertNil(warrenUpdateAvailableVersion(snapshot: snapshot, currentAppVersion: "2026.3"))
    }

    func testNoUpdateAvailableWithoutSnapshot() {
        XCTAssertNil(warrenUpdateAvailableVersion(snapshot: nil, currentAppVersion: "2026.3"))
    }

    // MARK: - Route decision

    func testBlockedUpdateRoutePreemptsEverything() {
        XCTAssertEqual(
            nextWarrenRoutes(
                updateBlocked: true,
                agreedToTOS: false,
                onboardingComplete: false,
                isRevoked: true,
                walletExists: false,
                isExpired: true
            ),
            [.blockedUpdate]
        )
    }

    // MARK: - Manifest verification through the Rust FFI

    /// Real production `ios.json` snapshot, signed by the pinned trusted
    /// metadata key (minimum_supported_version 2026.3, latest release
    /// 2026.3). Refresh from
    /// `https://api.warrenbrowse.com/updates/desktop/ios.json` when its
    /// `metadata_expiry` (2027-01-06) passes.
    private var fixtureManifest: Data {
        get throws {
            let url = try XCTUnwrap(
                Bundle(for: Self.self).url(forResource: "ios-manifest", withExtension: "json"),
                "ios-manifest.json fixture must be bundled with the test target"
            )
            return try Data(contentsOf: url)
        }
    }

    func testVerifierAcceptsSignedManifest() throws {
        let verified = try XCTUnwrap(
            WarrenVersionManifestVerifier.verify(manifest: fixtureManifest, currentVersion: "2026.3")
        )
        XCTAssertTrue(verified.supported)
        XCTAssertEqual(verified.latestVersion, "2026.3")
    }

    func testVerifierBlocksVersionBelowMinimum() throws {
        let verified = try XCTUnwrap(
            WarrenVersionManifestVerifier.verify(
                manifest: fixtureManifest,
                currentVersion: "2026.3-alpha1"
            )
        )
        XCTAssertFalse(verified.supported)
    }

    func testVerifierRejectsTamperedManifest() throws {
        var json = try XCTUnwrap(String(data: fixtureManifest, encoding: .utf8))
        json = json.replacingOccurrences(
            of: "\"minimum_supported_version\": \"2026.3\"",
            with: "\"minimum_supported_version\": \"9999.0.0\""
        )
        XCTAssertNil(
            WarrenVersionManifestVerifier.verify(
                manifest: Data(json.utf8),
                currentVersion: "2026.3"
            )
        )
    }

    func testVerifierRejectsGarbage() {
        XCTAssertNil(
            WarrenVersionManifestVerifier.verify(
                manifest: Data("not a manifest".utf8),
                currentVersion: "2026.3"
            )
        )
    }
}
