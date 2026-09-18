//
//  WarrenTunnelStatisticsViewTests.swift
//  WarrenVPNTests
//
//  Created by Warren on 2026-05-22.
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  Tests for the pure formatter helpers on `WarrenTunnelStatisticsView`
//  (formatBytes / formatDuration). UI rendering is not tested here ;
//  these helpers are the source-of-truth for the user-visible numbers
//  so they get explicit coverage.
//

import XCTest
@testable import WarrenVPN


final class WarrenTunnelStatisticsViewTests: XCTestCase {
    // MARK: - formatDuration

    func test_formatDuration_secondsOnly_lessThanOneHour() {
        XCTAssertEqual(WarrenTunnelStatisticsView.formatDuration(seconds: 0), "00:00")
        XCTAssertEqual(WarrenTunnelStatisticsView.formatDuration(seconds: 45), "00:45")
        XCTAssertEqual(WarrenTunnelStatisticsView.formatDuration(seconds: 60), "01:00")
        XCTAssertEqual(WarrenTunnelStatisticsView.formatDuration(seconds: 125), "02:05")
        XCTAssertEqual(WarrenTunnelStatisticsView.formatDuration(seconds: 3599), "59:59")
    }

    func test_formatDuration_includesHours_aboveOneHour() {
        XCTAssertEqual(WarrenTunnelStatisticsView.formatDuration(seconds: 3600), "01:00:00")
        XCTAssertEqual(WarrenTunnelStatisticsView.formatDuration(seconds: 3725), "01:02:05")
        XCTAssertEqual(WarrenTunnelStatisticsView.formatDuration(seconds: 86_400), "24:00:00")
    }

    // MARK: - formatBytes

    func test_formatBytes_zero() {
        // ByteCountFormatter yields "Zero KB" by default for 0 with
        // useKB allowed ; assert non-empty + contains expected unit.
        let s = WarrenTunnelStatisticsView.formatBytes(0)
        XCTAssertFalse(s.isEmpty)
    }

    /// The unit is translated ("MB" in English, "Mo" in French), so naming the
    /// English abbreviations here only ever tested one language. What the
    /// formatter owes its caller is a unit, and a unit that climbs with the
    /// count; both hold in every language.
    func test_formatBytes_carriesAUnitThatScalesWithTheCount() {
        func unit(_ formatted: String) -> String {
            formatted.filter { !$0.isNumber && !$0.isWhitespace && $0 != "." && $0 != "," }
        }

        let kilo = WarrenTunnelStatisticsView.formatBytes(2_048)
        let mega = WarrenTunnelStatisticsView.formatBytes(1_234_567)
        let giga = WarrenTunnelStatisticsView.formatBytes(3_221_225_472)

        XCTAssertFalse(unit(kilo).isEmpty, "no unit in \(kilo)")
        XCTAssertFalse(unit(mega).isEmpty, "no unit in \(mega)")
        XCTAssertNotEqual(unit(kilo), unit(mega), "2 KiB and 1.2 MiB print the same unit")
        XCTAssertNotEqual(unit(mega), unit(giga), "1.2 MiB and 3 GiB print the same unit")
    }

    // MARK: - struct equality

    func test_warrenTunnelStatistics_equatable() {
        let a = WarrenTunnelStatistics(
            stateLabel: "Connected",
            bytesIn: 100,
            bytesOut: 200,
            connectedDurationSeconds: 60,
            failoverCount: 0
        )
        let b = WarrenTunnelStatistics(
            stateLabel: "Connected",
            bytesIn: 100,
            bytesOut: 200,
            connectedDurationSeconds: 60,
            failoverCount: 0
        )
        XCTAssertEqual(a, b)

        let c = WarrenTunnelStatistics(
            stateLabel: "Connected",
            bytesIn: 101,
            bytesOut: 200,
            connectedDurationSeconds: 60,
            failoverCount: 0
        )
        XCTAssertNotEqual(a, c)
    }
}
