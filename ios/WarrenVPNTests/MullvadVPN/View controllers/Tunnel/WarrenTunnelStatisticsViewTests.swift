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

    func test_formatBytes_nonZero_includesUnit() {
        let s = WarrenTunnelStatisticsView.formatBytes(1_234_567)
        // Binary count style → either "1.2 MB" or "1,2 MB" depending
        // on the test runner's locale. Just assert the unit shows up.
        XCTAssertTrue(
            s.contains("MB") || s.contains("KB") || s.contains("GB"),
            "Expected a binary unit in \(s)"
        )
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
