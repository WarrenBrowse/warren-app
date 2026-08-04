//
//  SceneryScreenshotQA.swift
//  WarrenVPNUITests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  On-demand visual QA driver for the Bula connect screen: provisions a
//  local wallet (no backend), walks the tunnel state machine (the simulator
//  fakes the tunnel) and writes full-screen captures to the directory given
//  in WARREN_QA_SHOT_DIR, for a human or an agent to inspect. Run with:
//
//    xcodebuild test -scheme WarrenVPNUITests \
//      -only-testing:WarrenVPNUITests/SceneryScreenshotQA \
//      TEST_RUNNER_WARREN_QA_SHOT_DIR=/tmp/warren-shots
//
//  Without that variable the test skips, so plan-wide runs are unaffected.
//

import Foundation
import XCTest

final class SceneryScreenshotQA: WalletFlowUITestCase {
    private var shotDir: String!

    private func snap(_ name: String) {
        let data = XCUIScreen.main.screenshot().pngRepresentation
        let url = URL(fileURLWithPath: shotDir).appendingPathComponent("\(name).png")
        XCTAssertNoThrow(try data.write(to: url), "Failed writing \(url.path)")
    }

    /// Sleeps without blocking the run loop, so in-flight UI animations
    /// (crossfade, blur, zoom) actually progress while we wait.
    private func pause(_ seconds: TimeInterval) {
        let expectation = XCTestExpectation(description: "pause")
        DispatchQueue.global().asyncAfter(deadline: .now() + seconds) { expectation.fulfill() }
        _ = XCTWaiter.wait(for: [expectation], timeout: seconds + 2)
    }

    func testSceneryPhaseSweep() throws {
        guard let dir = ProcessInfo.processInfo.environment["WARREN_QA_SHOT_DIR"] else {
            throw XCTSkip("WARREN_QA_SHOT_DIR not set; this sweep is on-demand visual QA only")
        }
        shotDir = dir
        try FileManager.default.createDirectory(atPath: dir, withIntermediateDirectories: true)

        XCTAssertTrue(
            landsOnWalletLoginChooser(),
            "Expected the wallet login chooser (onboarding-complete is pre-seeded and no wallet exists)"
        )
        snap("00-wallet-chooser")

        WalletLoginPage(app)
            .tapCreateWalletButton()
        WalletMnemonicDisplayPage(app)
            .tapConfirmWrittenDownButton()

        if app.otherElements[.notificationPromptView].existsAfterWait(timeout: .default) {
            NotificationPromptPage(app).tapSkipButton()
        }

        XCTAssertTrue(
            app.buttons[AccessibilityIdentifier.connectButton].existsAfterWait(timeout: .long),
            "Expected the connect screen after wallet provisioning"
        )
        // Let the scenery crossfade and the header settle before capturing.
        pause(2.0)
        snap("01-disconnected")

        TunnelControlPage(app).tapConnectButton()
        pause(0.7)
        snap("02-connecting-early")
        pause(2.5)
        snap("03-connecting-blur")

        XCTAssertTrue(
            app.staticTexts[AccessibilityIdentifier.connectionStatusConnectedLabel]
                .existsAfterWait(timeout: .veryLong),
            "Tunnel (simulated) should reach the connected state"
        )
        pause(2.0)
        snap("04-connected")

        TunnelControlPage(app).tapRelayStatusExpandCollapseButton()
        pause(1.2)
        snap("05-connected-expanded")
        TunnelControlPage(app).tapRelayStatusExpandCollapseButton()
        pause(0.8)

        TunnelControlPage(app).tapDisconnectButton()
        pause(0.8)
        snap("06-disconnecting")
        pause(1.7)
        snap("07-disconnected-again")
    }
}
