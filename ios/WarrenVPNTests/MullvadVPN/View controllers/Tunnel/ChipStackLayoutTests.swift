//
//  ChipStackLayoutTests.swift
//  WarrenVPNTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import XCTest

@testable import WarrenVPN

/// The feature pills are one stack with one gap on all three clients. iOS is
/// the client that drifted: it drew a wrapping horizontal flow whose rows were
/// 16 pt apart (an 8 pt vertical padding on each pill) with a "N more..."
/// button hiding the overflow, where desktop and Android both draw a
/// left-aligned column 5 apart with every pill visible.
///
/// The Android twin of this assertion is `DesignParityTest.kt`.
final class ChipStackLayoutTests: XCTestCase {
    func testTheStackGapIsTheFiveTheOtherTwoClientsUse() {
        XCTAssertEqual(UIMetrics.FeatureIndicators.chipStackGap, 5)
    }
}
