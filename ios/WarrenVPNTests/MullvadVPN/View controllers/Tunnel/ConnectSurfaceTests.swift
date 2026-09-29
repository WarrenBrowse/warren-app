//
//  ConnectSurfaceTests.swift
//  WarrenVPNTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import UIKit
import WarrenMockData
import WarrenRustRuntime
import WarrenSettings
import WarrenTypes
import XCTest

@testable import WarrenVPN

/// The connect screen's card, chips and banner are drawn from the palette and
/// the measurements desktop generates into `design-tokens.json`. iOS has no
/// generator, so its copy is held to that file here: a value changed on desktop
/// and not carried over fails this suite instead of drifting on a phone.
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
    func testTheLightCardIsNotTheDarkCard() {
        XCTAssertNotEqual(
            ConnectSurfacePalette(theme: .light).argb(.card),
            ConnectSurfacePalette(theme: .dark).argb(.card)
        )
    }

    func testTheColourCarriesTheAlphaOfItsToken() {
        var alpha: CGFloat = 0
        ConnectSurfacePalette(theme: .dark).uiColor(.shadowStrong)
            .getRed(nil, green: nil, blue: nil, alpha: &alpha)
        XCTAssertEqual(alpha, CGFloat(0x73) / 255, accuracy: 0.001)
    }

    func testEveryMeasurementMatchesTheSharedComponents() throws {
        let components = try XCTUnwrap(try loadTokens()["components"] as? [String: [String: Any]])
        typealias M = ConnectSurfaceMetrics
        let expected: [(String, CGFloat)] = [
            ("connectionCard.paddingHorizontal", M.Card.padding),
            ("connectionCard.paddingVertical", M.Card.padding),
            ("connectionCard.radius", M.Card.radius),
            ("connectionCard.borderWidth", M.Card.borderWidth),
            ("connectionCard.shadowOffsetY", M.Card.shadowOffsetY),
            ("connectionCard.shadowBlur", M.Card.shadowBlur),
            ("connectionCard.marginHorizontal", M.Card.marginHorizontal),
            ("connectionCard.marginBottom", M.Card.marginBottom),
            ("connectionCard.blockGap", M.Card.blockGap),
            ("connectionCard.badgeGap", M.Card.chipGap),
            ("connectionCard.badgesToCardGap", M.Card.chipsToCardGap),
            ("connectionCard.chevronButtonSize", M.Card.chevronBoxSize),
            ("connectionCard.chevronIconSize", M.Card.chevronIconSize),
            ("connectionStatus.rowGap", M.Status.rowGap),
            ("connectionStatus.textGap", M.Status.textGap),
            ("connectionStatus.trailingGap", M.Status.trailingGap),
            ("connectionStatus.wellSize", M.Status.wellSize),
            ("connectionStatus.wellRadius", M.Status.wellRadius),
            ("connectionStatus.iconSize", M.Status.iconSize),
            ("connectionStatus.titleSize", M.Status.titleSize),
            ("connectionStatus.titleLineHeight", M.Status.titleLineHeight),
            ("connectionStatus.subtitleSize", M.Status.subtitleSize),
            ("connectionStatus.subtitleLineHeight", M.Status.subtitleLineHeight),
            ("connectionLocation.gapAbove", M.Location.gapAbove),
            ("connectionLocation.size", M.Location.size),
            ("connectionLocation.lineHeight", M.Location.lineHeight),
            ("connectionLocation.hostnameGapAbove", M.Location.hostnameGapAbove),
            ("connectionLocation.hostnameSize", M.Location.hostnameSize),
            ("connectionLocation.hostnameLineHeight", M.Location.hostnameLineHeight),
            ("cardButton.height", M.Button.height),
            ("cardButton.radius", M.Button.radius),
            ("cardButton.borderWidth", M.Button.borderWidth),
            ("cardButton.textSize", M.Button.textSize),
            ("cardButton.textLineHeight", M.Button.textLineHeight),
            ("cardButton.rowGap", M.Button.rowGap),
            ("cardButton.shuffleWidth", M.Button.shuffleWidth),
            ("featureChip.paddingVertical", M.Chip.paddingVertical),
            ("featureChip.paddingHorizontal", M.Chip.paddingHorizontal),
            ("featureChip.radius", M.Chip.radius),
            ("featureChip.borderWidth", M.Chip.borderWidth),
            ("featureChip.shadowOffsetY", M.Chip.shadowOffsetY),
            ("featureChip.shadowBlur", M.Chip.shadowBlur),
            ("countryFlag.size", M.Flag.size),
            ("countryFlag.ringWidth", M.Flag.ringWidth),
            ("betaBanner.height", M.Banner.height),
            ("betaBanner.paddingStart", M.Banner.paddingStart),
            ("betaBanner.paddingEnd", M.Banner.paddingEnd),
            ("betaBanner.gap", M.Banner.gap),
            ("betaBanner.radius", M.Banner.radius),
            ("betaBanner.borderWidth", M.Banner.borderWidth),
            ("betaBanner.shadowOffsetY", M.Banner.shadowOffsetY),
            ("betaBanner.shadowBlur", M.Banner.shadowBlur),
            ("betaBanner.pillHeight", M.Banner.pillHeight),
            ("betaBanner.pillPaddingHorizontal", M.Banner.pillPaddingHorizontal),
            ("betaBanner.pillRadius", M.Banner.pillRadius),
            ("betaBanner.pillTextSize", M.Banner.pillTextSize),
            ("betaBanner.pillLetterSpacing", M.Banner.pillLetterSpacing),
            ("betaBanner.textSize", M.Banner.textSize),
        ]

        for (path, value) in expected {
            let parts = path.split(separator: ".").map(String.init)
            let entry = try XCTUnwrap(components[parts[0]]?[parts[1]] as? [String: Any], "\(path) left the table")
            let shared = try XCTUnwrap(entry["value"] as? Double, path)
            XCTAssertEqual(Double(value), shared, accuracy: 0.0001, path)
        }
    }

    /// The card writes the phase in the surface palette, desktop's
    /// `phaseCardColors` in `connection-phase.ts`: the title carries the hue,
    /// the well behind the eye is a quiet fill of it, and the kill-switch
    /// state stays neutral.
    func testEachPhaseTakesItsCardColours() {
        let expected: [(ConnectionPhase, ConnectSurfaceToken, ConnectSurfaceToken)] = [
            (.exposed, .exposed, .exposedWell),
            (.connecting, .connecting, .connectingWell),
            (.protected, .protected, .protectedWell),
            (.interrupted, .connecting, .connectingWell),
            (.blocked, .text, .button),
        ]
        for (phase, title, well) in expected {
            XCTAssertEqual(phase.cardTitleToken, title, "\(phase) title")
            XCTAssertEqual(phase.cardWellToken, well, "\(phase) well")
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

    /// The action is filled with what the tap does, never with the phase hue.
    func testTheActionIsFilledWithWhatTheTapDoes() {
        XCTAssertEqual(model(.disconnected).actionButtonFillToken, .connect)
        XCTAssertEqual(
            model(.connecting(nil, isPostQuantum: false, isDaita: false)).actionButtonFillToken,
            .cancel
        )
        XCTAssertEqual(model(.disconnecting(.reconnect)).actionButtonFillToken, .cancel)
    }

    /// The blocked state's cancel disconnects and is labelled Disconnect, so
    /// it takes the disconnect fill rather than the cancel one.
    func testTheBlockedCancelTakesTheDisconnectFill() {
        let blocked = model(.waitingForConnectivity(.noConnection))
        XCTAssertEqual(blocked.actionButton, .cancel)
        XCTAssertEqual(blocked.actionButtonFillToken, .disconnect)
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
