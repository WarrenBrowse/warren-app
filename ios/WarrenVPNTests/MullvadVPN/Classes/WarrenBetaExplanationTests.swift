//
//  WarrenBetaExplanationTests.swift
//  WarrenVPNTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import WarrenRustRuntime
import XCTest

@testable import WarrenVPN

/// What a marked build tells its user about the network it is on.
final class WarrenBetaExplanationTests: XCTestCase {
    private func networkInfo(capBps: UInt64?) -> WarrenNetworkInfo {
        WarrenNetworkInfo(
            environment: "beta", degraded: true, defaultRateBps: capBps, paymentsEnabled: false)
    }

    func testTheCapIsNamedWhenTheServerGaveOne() {
        let summary = WarrenBetaExplanation.summary(networkInfo: networkInfo(capBps: 20_000_000))
        XCTAssertTrue(summary.contains("20"), summary)

        let paragraphs = WarrenBetaExplanation.paragraphs(networkInfo: networkInfo(capBps: 20_000_000))
        XCTAssertTrue(paragraphs.contains { $0.contains("20") }, "\(paragraphs)")
    }

    /// A figure the user can check and find wrong is worse than none, so a
    /// network that named no cap is described as limited and nothing more.
    func testWithNoCapTheTextNamesNoFigureAtAll() {
        for info in [nil, networkInfo(capBps: nil), networkInfo(capBps: 0)] {
            let summary = WarrenBetaExplanation.summary(networkInfo: info)
            XCTAssertFalse(summary.contains(where: \.isNumber), summary)
            for paragraph in WarrenBetaExplanation.paragraphs(networkInfo: info) {
                XCTAssertFalse(paragraph.contains(where: \.isNumber), paragraph)
            }
        }
    }

    /// Three ideas, each its own paragraph: what this build is, what it costs
    /// in speed, and that the conditions are temporary. A user who reads only
    /// the first would otherwise take the beta for the product.
    func testTheExplanationSaysWhatItIsWhatItCostsAndThatItIsTemporary() {
        let paragraphs = WarrenBetaExplanation.paragraphs(networkInfo: networkInfo(capBps: 20_000_000))

        XCTAssertEqual(paragraphs.count, 3)
        for paragraph in paragraphs {
            XCTAssertFalse(paragraph.isEmpty)
        }
        XCTAssertFalse(WarrenBetaExplanation.title().isEmpty)
    }
}
