//
//  ConnectSurfaceTests.swift
//  WarrenVPNTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import PacketTunnelCore
import UIKit
import WarrenMockData
import WarrenRustRuntime
import WarrenSettings
import WarrenTypes
import XCTest

@testable import WarrenVPN

/// The home is drawn from the palette desktop generates into
/// `design-tokens.json`. iOS has no generator, so its copy is held to that file
/// here: a value changed on desktop and not carried over fails this suite
/// instead of drifting on a phone. The geometry is `HomeMetricsTests`'.
final class ConnectSurfaceTests: XCTestCase {
    /// `ios/WarrenVPNTests/MullvadVPN/View controllers/Tunnel/ConnectSurfaceTests.swift`
    private static let repoDir = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .deletingLastPathComponent()

    private func loadTokens() throws -> [String: Any] {
        let data = try Data(contentsOf: Self.repoDir.appendingPathComponent("design-tokens.json"))
        return try XCTUnwrap(try JSONSerialization.jsonObject(with: data) as? [String: Any])
    }

    private func argb(_ hex: String) throws -> UInt32 {
        XCTAssertTrue(hex.hasPrefix("#") && hex.count == 9, "not #AARRGGBB: \(hex)")
        return try XCTUnwrap(UInt32(hex.dropFirst(), radix: 16))
    }

    func testEverySurfaceMatchesTheSharedTokensInBothThemes() throws {
        let surfaces = try XCTUnwrap(try loadTokens()["surfaces"] as? [String: [String: String]])

        for theme in [WarrenTheme.dark, .light] {
            let name = theme == .dark ? "dark" : "light"
            let table = try XCTUnwrap(surfaces[name], "no \(name) surfaces")
            for token in ConnectSurfaceToken.allCases {
                let hex = try XCTUnwrap(table[token.rawValue], "\(name).\(token.rawValue) left the shared table")
                XCTAssertEqual(
                    ConnectSurfacePalette(theme: theme).argb(token),
                    try argb(hex),
                    "\(name).\(token.rawValue)"
                )
            }
        }
    }

    /// The two themes are two palettes: a token that reads the same in both
    /// would mean the light copy was never filled in.
    func testTheLightPageIsNotTheDarkPage() {
        XCTAssertNotEqual(
            ConnectSurfacePalette(theme: .light).argb(.homePage),
            ConnectSurfacePalette(theme: .dark).argb(.homePage)
        )
    }

    func testTheColourCarriesTheAlphaOfItsToken() {
        var alpha: CGFloat = 0
        ConnectSurfacePalette(theme: .dark).uiColor(.homeToolbar)
            .getRed(nil, green: nil, blue: nil, alpha: &alpha)
        XCTAssertEqual(alpha, CGFloat(0xB8) / 255, accuracy: 0.001)
    }

    /// The dynamic colour resolves to the palette of the style it is drawn in.
    func testTheDynamicColourFollowsTheInterfaceStyle() {
        let token = ConnectSurfaceToken.homeText
        let light = ConnectSurfacePalette.dynamic(token)
            .resolvedColor(with: UITraitCollection(userInterfaceStyle: .light))
        let dark = ConnectSurfacePalette.dynamic(token)
            .resolvedColor(with: UITraitCollection(userInterfaceStyle: .dark))
        XCTAssertEqual(light, ConnectSurfacePalette(theme: .light).uiColor(token))
        XCTAssertEqual(dark, ConnectSurfacePalette(theme: .dark).uiColor(token))
    }

    /// The pill takes desktop's `pillColors`: the pending colour while the
    /// tunnel comes up and while it waits for a network, a neutral one for the
    /// kill switch.
    func testEachPhaseTakesItsPillColours() {
        let expected: [(ConnectionPhase, ConnectSurfaceToken, ConnectSurfaceToken)] = [
            (.exposed, .homePillExposed, .homePillExposedText),
            (.connecting, .homePillPending, .homePillPendingText),
            (.protected, .homePillProtected, .homePillProtectedText),
            (.interrupted, .homePillPending, .homePillPendingText),
            (.blocked, .homePillBlocked, .homePillBlockedText),
        ]
        for (phase, background, text) in expected {
            XCTAssertEqual(phase.homePillTokens.background, background, "\(phase) background")
            XCTAssertEqual(phase.homePillTokens.text, text, "\(phase) text")
        }
    }

    private func model(_ state: TunnelState) -> ConnectionViewViewModel {
        ConnectionViewViewModel(
            tunnelStatus: TunnelStatus(state: state),
            relayConstraints: RelayConstraints(),
            relayCache: MockRelayCache(),
            customListRepository: CustomListRepository()
        )
    }

    /// The action says what the tap does: a solid go, an outlined stop, an
    /// amber cancel while connecting.
    func testTheActionLooksLikeWhatTheTapDoes() {
        XCTAssertEqual(model(.disconnected).homeActionTone, .connect)
        XCTAssertEqual(model(.connecting(nil, isPostQuantum: false, isDaita: false)).homeActionTone, .cancel)
        XCTAssertEqual(model(.disconnecting(.reconnect)).homeActionTone, .cancel)
        XCTAssertEqual(
            model(.connected(HomeStatusTests.helsinki(), isPostQuantum: false, isDaita: false)).homeActionTone,
            .disconnect
        )
    }

    /// While the kill switch holds the traffic the action turns it off, a
    /// neutral one, whichever button the state offers.
    func testTheBlockedActionIsNeutral() {
        let blocked = model(.waitingForConnectivity(.noConnection))
        XCTAssertEqual(blocked.actionButton, .cancel)
        XCTAssertEqual(blocked.homeActionTone, .neutral)
        XCTAssertEqual(model(.error(.noRelaysSatisfyingConstraints)).homeActionTone, .neutral)
    }

    /// The banner says "limited" until the server names its cap, then the cap.
    func testTheBannerNamesTheCapOnceTheServerGivesIt() {
        let viewModel = model(.disconnected)
        XCTAssertEqual(viewModel.productBannerLine, WarrenBetaExplanation.summary(networkInfo: nil))

        viewModel.networkInfo = WarrenNetworkInfo(
            environment: "beta",
            degraded: true,
            defaultRateBps: 20_000_000,
            paymentsEnabled: false
        )
        XCTAssertTrue(viewModel.productBannerLine.contains("20"), viewModel.productBannerLine)
    }
}
