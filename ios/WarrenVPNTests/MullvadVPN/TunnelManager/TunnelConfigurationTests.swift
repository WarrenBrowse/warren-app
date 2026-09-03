//
//  TunnelConfigurationTests.swift
//  WarrenVPNTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import NetworkExtension
import XCTest

@testable import WarrenRustRuntime
@testable import WarrenSettings
@testable import WarrenVPN

/// The VPN row iOS shows in Settings, General, VPN and Device Management is the
/// only status surface the app can brand (the status-bar chip is the system's,
/// with no app-supplied content), so with a beta install sitting beside a prod
/// one it is the single place that says which product holds the tunnel.
final class TunnelConfigurationTests: XCTestCase {
    private func anchors(_ environment: String) throws -> WarrenProductAnchors {
        let fixture = try ClientRulesFixtures.load("product_env.json")
        let environments = try ClientRulesFixtures.object(fixture, "environments")
        let row = try ClientRulesFixtures.object(environments, environment)
        let data = try JSONSerialization.data(withJSONObject: row)
        return try XCTUnwrap(WarrenProductAnchors.decode(String(decoding: data, as: UTF8.self)))
    }

    /// Prod's name must not move: it is written into the row of every install
    /// that already has a configuration, and a rename rewrites it.
    func testProdKeepsTheShippedName() throws {
        XCTAssertEqual(TunnelConfiguration.vpnConfigurationName(for: try anchors("prod")), "QUIC")
    }

    func testEveryOtherEnvironmentIsMarkedInTheRow() throws {
        XCTAssertEqual(TunnelConfiguration.vpnConfigurationName(for: try anchors("beta")), "QUIC (Beta)")
        XCTAssertEqual(
            TunnelConfiguration.vpnConfigurationName(for: try anchors("staging")),
            "QUIC (Staging)"
        )
    }

    /// The name is taken from the compiled product table, not spelled again in
    /// the initializer: a literal there is exactly the bug this replaces.
    func testTheConfigurationTakesItsNameFromTheCompiledEnvironment() {
        let configuration = TunnelConfiguration(includeAllNetworks: false, excludeLocalNetworks: false)
        XCTAssertEqual(
            configuration.localizedDescription,
            TunnelConfiguration.vpnConfigurationName(for: .current)
        )
    }

    /// `apply(to:)` is the single write of the whole configuration, and both
    /// call sites save the manager to preferences right after it, so nothing
    /// else has to carry the name to the system.
    func testApplyWritesTheNameOntoTheManager() {
        let manager = TunnelProviderManagerType()
        let configuration = TunnelConfiguration(includeAllNetworks: false, excludeLocalNetworks: false)

        configuration.apply(to: manager)

        XCTAssertEqual(manager.localizedDescription, configuration.localizedDescription)
        XCTAssertEqual(manager.isEnabled, configuration.isEnabled)
        XCTAssertEqual(manager.isOnDemandEnabled, configuration.isOnDemandEnabled)
    }

    /// An armed `NEOnDemandRuleConnect` brings the tunnel up with no tap at
    /// all, so a build that has stood down for a higher-priority environment
    /// would come straight back under its own stand-down banner.
    func testAConfigurationWrittenWhileStandingDownArmsNoOnDemandRule() {
        let manager = TunnelProviderManagerType()
        let configuration = TunnelConfiguration(
            includeAllNetworks: false,
            excludeLocalNetworks: false,
            standDown: WarrenEnvStandDownRecord(
                higherEnvironmentSeen: "prod",
                reEnabled: false,
                restoreIncludeAllNetworks: false
            )
        )

        configuration.apply(to: manager)

        XCTAssertFalse(configuration.isOnDemandEnabled)
        XCTAssertFalse(manager.isOnDemandEnabled)
    }

    /// The user asked for this build back, so the rule that reconnects it
    /// without a tap comes back with it: a re-enabled record is not a
    /// stand-down.
    func testAReEnabledRecordArmsTheOnDemandRuleAgain() {
        let configuration = TunnelConfiguration(
            includeAllNetworks: false,
            excludeLocalNetworks: false,
            standDown: WarrenEnvStandDownRecord(
                higherEnvironmentSeen: "prod",
                reEnabled: true,
                restoreIncludeAllNetworks: false
            )
        )

        XCTAssertTrue(configuration.isOnDemandEnabled)
    }
}
