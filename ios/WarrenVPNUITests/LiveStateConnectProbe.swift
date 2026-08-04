//
//  LiveStateConnectProbe.swift
//  WarrenVPNUITests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  Diagnostic probe, NOT a test of app logic: launches the app WITHOUT
//  any reset launch arguments (subclasses XCTestCase directly, never
//  BaseUITestCase), so the user's live settings, wallet and caches are
//  preserved, then taps Connect once and screenshots the outcome. Used
//  to reproduce a connection failure with the exact on-device state.
//  Skipped unless WARREN_QA_LIVE_PROBE=1 (pass as
//  TEST_RUNNER_WARREN_QA_LIVE_PROBE to xcodebuild).
//

import Foundation
import XCTest

final class LiveStateConnectProbe: XCTestCase {
    func testConnectOnceWithLiveState() throws {
        guard ProcessInfo.processInfo.environment["WARREN_QA_LIVE_PROBE"] == "1" else {
            throw XCTSkip("Set TEST_RUNNER_WARREN_QA_LIVE_PROBE=1 to run the live-state probe")
        }

        let app = XCUIApplication()
        app.launch()

        let connect = app.buttons[AccessibilityIdentifier.connectButton]
        guard connect.waitForExistence(timeout: 15) else {
            snap(app, "probe-not-on-main")
            XCTFail("Connect button not found; the app is not on the main screen")
            return
        }

        // Optionally pick a country first (overwrites a stale stored
        // location constraint); selecting a location starts the tunnel.
        if let country = ProcessInfo.processInfo.environment["WARREN_QA_PICK_COUNTRY"],
           !country.isEmpty {
            app.buttons[AccessibilityIdentifier.selectLocationButton].tap()
            let cell = app.cells.containing(NSPredicate(format: "label CONTAINS %@", country))
                .firstMatch
            guard cell.waitForExistence(timeout: 10) else {
                snap(app, "probe-no-country-cell")
                XCTFail("No location cell matching \(country)")
                return
            }
            cell.tap()
        } else {
            connect.tap()
        }

        // Give the (simulator) tunnel time to settle in either state.
        Thread.sleep(forTimeInterval: 6)
        snap(app, "probe-after-connect")
    }

    // Opens the account view on the live state and screenshots it.
    // Skipped unless WARREN_QA_ACCOUNT_PROBE=1.
    func testAccountPageSnapshotWithLiveState() throws {
        guard ProcessInfo.processInfo.environment["WARREN_QA_ACCOUNT_PROBE"] == "1" else {
            throw XCTSkip("Set TEST_RUNNER_WARREN_QA_ACCOUNT_PROBE=1 to run the account probe")
        }

        let app = XCUIApplication()
        app.launch()

        let accountButton = app.buttons[AccessibilityIdentifier.accountButton]
        guard accountButton.waitForExistence(timeout: 15) else {
            snap(app, "account-probe-no-header")
            XCTFail("Account header button not found; the app is not on the main screen")
            return
        }
        accountButton.tap()

        guard app.otherElements[AccessibilityIdentifier.accountView].waitForExistence(timeout: 10)
        else {
            snap(app, "account-probe-no-account-view")
            XCTFail("Account view did not appear")
            return
        }
        Thread.sleep(forTimeInterval: 1)
        snap(app, "account-probe")
    }

    // Taps the out-of-time purchase button on the live state and
    // screenshots the outcome. Skipped unless WARREN_QA_OOT_PROBE=1.
    func testOutOfTimePurchaseSnapshotWithLiveState() throws {
        guard ProcessInfo.processInfo.environment["WARREN_QA_OOT_PROBE"] == "1" else {
            throw XCTSkip("Set TEST_RUNNER_WARREN_QA_OOT_PROBE=1 to run the out-of-time probe")
        }

        let app = XCUIApplication()
        app.launch()

        guard app.otherElements[AccessibilityIdentifier.outOfTimeView].waitForExistence(timeout: 15)
        else {
            snap(app, "oot-probe-not-gated")
            XCTFail("Out-of-time screen not shown; the live account is not expired")
            return
        }
        app.buttons[AccessibilityIdentifier.purchaseButton].tap()
        Thread.sleep(forTimeInterval: 8)
        snap(app, "oot-probe-after-purchase-tap")
    }

    private func snap(_ app: XCUIApplication, _ name: String) {
        guard let dir = ProcessInfo.processInfo.environment["WARREN_QA_SHOT_DIR"] else { return }
        let png = XCUIScreen.main.screenshot().pngRepresentation
        try? png.write(to: URL(fileURLWithPath: dir).appendingPathComponent("\(name).png"))
    }
}
