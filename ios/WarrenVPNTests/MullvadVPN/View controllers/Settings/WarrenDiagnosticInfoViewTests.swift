//
//  WarrenDiagnosticInfoViewTests.swift
//  WarrenVPNTests
//
//  Created by Warren on 2026-05-22.
//  Copyright Â© 2026 Warren Browse. All rights reserved.
//
//  Tests for the support-ticket plain-text rendering. UI is not
//  tested ; the `plainTextSummary` is the user-visible payload pasted
//  into support tickets so explicit coverage matters.
//

import XCTest

@testable import WarrenVPN

final class WarrenDiagnosticInfoViewTests: XCTestCase {
    private func makeStats(
        state: String = "Connected",
        bytesIn: UInt64 = 0,
        bytesOut: UInt64 = 0,
        duration: UInt64? = nil,
        failovers: UInt32 = 0
    ) -> WarrenTunnelStatistics {
        WarrenTunnelStatistics(
            stateLabel: state,
            bytesIn: bytesIn,
            bytesOut: bytesOut,
            connectedDurationSeconds: duration,
            failoverCount: failovers
        )
    }

    func test_plainTextSummary_includesAllExpectedLines_whenPubkeyPresent() {
        let info = WarrenDiagnosticInfo(
            appVersion: "0.5.2",
            buildNumber: "1042",
            walletAddressShort: "wb7kgy…hP9DnB",
            tunnelStats: makeStats(
                state: "Connected",
                bytesIn: 100,
                bytesOut: 200,
                duration: 65,
                failovers: 1
            )
        )
        let text = WarrenDiagnosticInfoView.plainTextSummary(info)
        XCTAssertTrue(text.contains("Warren VPN v0.5.2 (build 1042)"))
        XCTAssertTrue(text.contains("Wallet ID: wb7kgy…hP9DnB"))
        XCTAssertTrue(text.contains("Status: Connected"))
        XCTAssertTrue(text.contains("Connected for: 01:05"))
        XCTAssertTrue(text.contains("Failovers: 1"))
    }

    func test_plainTextSummary_omitsWalletLine_whenPubkeyNil() {
        let info = WarrenDiagnosticInfo(
            appVersion: "1.0.0",
            buildNumber: "1",
            walletAddressShort: nil,
            tunnelStats: makeStats()
        )
        let text = WarrenDiagnosticInfoView.plainTextSummary(info)
        XCTAssertFalse(text.contains("Wallet ID"))
        XCTAssertTrue(text.contains("Warren VPN v1.0.0"))
        XCTAssertTrue(text.contains("Status: Connected"))
    }

    func test_plainTextSummary_omitsDurationLine_whenStatsDurationNil() {
        let info = WarrenDiagnosticInfo(
            appVersion: "1.0.0",
            buildNumber: "1",
            walletAddressShort: nil,
            tunnelStats: makeStats(state: "Disconnected", duration: nil)
        )
        let text = WarrenDiagnosticInfoView.plainTextSummary(info)
        XCTAssertFalse(text.contains("Connected for"))
    }

    func test_plainTextSummary_isMultiLineNewlineSeparated() {
        let info = WarrenDiagnosticInfo(
            appVersion: "1.0.0",
            buildNumber: "1",
            walletAddressShort: "wbAbcd…Xyz789",
            tunnelStats: makeStats(state: "Connected", duration: 30)
        )
        let text = WarrenDiagnosticInfoView.plainTextSummary(info)
        let lines = text.components(separatedBy: "\n")
        // Warren VPN + Wallet ID + Status + Connected for + Data in + Data out + Failovers = 7
        XCTAssertEqual(lines.count, 7, "Expected 7 lines, got \(lines.count) : \(lines)")
    }
}
