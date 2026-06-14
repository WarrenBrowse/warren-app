//
//  StoreSubscriptionTests.swift
//  WarrenVPNTests
//
//  Created by Warren on 2026-06-14.
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  Pins the product-id <-> duration contract between StoreKit and the
//  warren-api backend duration map. A typo in a raw value here, or a
//  drift in the months mapping, silently desyncs the app from the
//  backend `[mobile.apple] products` map (rejected at credit time with
//  HTTP 422). These tests fail fast so the desync is caught in CI
//  instead of at purchase time.
//

import XCTest
@testable import WarrenVPN

final class StoreSubscriptionTests: XCTestCase {
    func test_allCases_hasExactlyFourProducts() {
        XCTAssertEqual(
            StoreSubscription.allCases.count,
            4,
            "Add the matching product in App Store Connect AND the warren-api duration map before expanding allCases"
        )
    }

    func test_rawValues_exactlyMatchBackendProductIds() {
        XCTAssertEqual(StoreSubscription.oneMonth.rawValue, "net.warrenbrowse.vpn.timeadd.1month")
        XCTAssertEqual(StoreSubscription.threeMonths.rawValue, "net.warrenbrowse.vpn.timeadd.3months")
        XCTAssertEqual(StoreSubscription.sixMonths.rawValue, "net.warrenbrowse.vpn.timeadd.6months")
        XCTAssertEqual(StoreSubscription.twelveMonths.rawValue, "net.warrenbrowse.vpn.timeadd.12months")
    }

    func test_months_matchProductDurations() {
        XCTAssertEqual(StoreSubscription.oneMonth.months, 1)
        XCTAssertEqual(StoreSubscription.threeMonths.months, 3)
        XCTAssertEqual(StoreSubscription.sixMonths.months, 6)
        XCTAssertEqual(StoreSubscription.twelveMonths.months, 12)
    }

    func test_rawValues_areUnique() {
        let rawValues = StoreSubscription.allCases.map(\.rawValue)
        XCTAssertEqual(
            rawValues.count,
            Set(rawValues).count,
            "Duplicate product IDs would map two durations to the same StoreKit product"
        )
    }

    func test_initFromRawValue_roundTrips() {
        for product in StoreSubscription.allCases {
            XCTAssertEqual(
                StoreSubscription(rawValue: product.rawValue),
                product,
                "Raw value round-trip failed for \(product)"
            )
        }
    }

    func test_initFromRawValue_returnsNil_forUnknownProductId() {
        XCTAssertNil(StoreSubscription(rawValue: "net.warrenbrowse.vpn.timeadd.99months"))
        XCTAssertNil(StoreSubscription(rawValue: ""))
    }

    func test_localizedTitle_containsDisplayPrice() {
        let price = "$4.99"
        for product in StoreSubscription.allCases {
            let title = product.localizedTitle(displayPrice: price)
            XCTAssertTrue(
                title.contains(price),
                "localizedTitle for \(product) must embed the display price, got: \(title)"
            )
        }
    }

    func test_localizedTitle_containsMonthCount() {
        XCTAssertTrue(StoreSubscription.oneMonth.localizedTitle(displayPrice: "$1").contains("1"))
        XCTAssertTrue(StoreSubscription.threeMonths.localizedTitle(displayPrice: "$1").contains("3"))
        XCTAssertTrue(StoreSubscription.sixMonths.localizedTitle(displayPrice: "$1").contains("6"))
        XCTAssertTrue(StoreSubscription.twelveMonths.localizedTitle(displayPrice: "$1").contains("12"))
    }
}
