//
//  HomeTimeLeftTests.swift
//  WarrenVPNTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import XCTest

@testable import WarrenVPN

/// The footer's "N days left", as desktop's `formatRemainingTime(expiry, { suffix: true })` writes
/// it, and the footer giving way to the renewal notice near expiry.
final class HomeTimeLeftTests: XCTestCase {
    private var calendar: Calendar = {
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = TimeZone(identifier: "UTC")!
        return calendar
    }()

    private let now = Date(timeIntervalSince1970: 1_800_000_000)

    private func days(_ count: Double) -> Date {
        now.addingTimeInterval(count * 86_400)
    }

    func testTheFooterCountsTheWholeDaysLeft() {
        XCTAssertEqual(HomeTimeLeft.text(now: now, expiry: days(14.5), calendar: calendar), "14 days left")
    }

    func testOneDayIsSingular() {
        XCTAssertEqual(HomeTimeLeft.remaining(from: now, to: days(1.2), calendar: calendar), "1 day left")
    }

    func testLessThanADayIsSaidSo() {
        XCTAssertEqual(HomeTimeLeft.remaining(from: now, to: days(0.5), calendar: calendar), "Less than a day left")
    }

    func testFromTwoYearsOnTheFooterCountsYears() {
        XCTAssertEqual(HomeTimeLeft.text(now: now, expiry: days(731), calendar: calendar), "2 years left")
        XCTAssertEqual(HomeTimeLeft.text(now: now, expiry: days(729), calendar: calendar), "729 days left")
    }

    /// Close to expiry the notification area carries the renewal call, and an expired account has
    /// nothing left to count.
    func testTheFooterGivesWayNearAndAfterTheExpiry() {
        XCTAssertNil(HomeTimeLeft.text(now: now, expiry: days(2.9), calendar: calendar))
        XCTAssertNotNil(HomeTimeLeft.text(now: now, expiry: days(3.1), calendar: calendar))
        XCTAssertNil(HomeTimeLeft.text(now: now, expiry: days(-1), calendar: calendar))
        XCTAssertNil(HomeTimeLeft.text(now: now, expiry: nil, calendar: calendar))
    }
}
