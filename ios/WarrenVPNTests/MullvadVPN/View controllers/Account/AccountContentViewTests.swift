//
//  AccountContentViewTests.swift
//  WarrenVPNTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import XCTest

@testable import WarrenVPN

final class AccountContentViewTests: XCTestCase {
    func testActiveSubscriptionShowsRemainingTimeHeadline() {
        let contentView = AccountContentView()

        contentView.setExpiry(Date().addingTimeInterval(86400 * 30))

        XCTAssertFalse(contentView.remainingTimeLabel.isHidden)
        XCTAssertFalse(contentView.remainingTimeLabel.text?.isEmpty ?? true)
    }

    func testExpiredSubscriptionHidesHeadlineAndMarksRowOutOfTime() {
        let contentView = AccountContentView()

        contentView.setExpiry(Date(timeIntervalSince1970: 0))

        // The red OUT OF TIME state lives in the paid-until row only; the
        // big headline disappears (desktop behavior), so the state is
        // never shown twice on the same card.
        XCTAssertTrue(contentView.remainingTimeLabel.isHidden)
        XCTAssertEqual(
            contentView.accountExpiryRowView.accessibilityValue,
            NSLocalizedString("OUT OF TIME", comment: "")
        )
    }

    func testUnknownExpiryShowsUnavailablePlaceholder() {
        let contentView = AccountContentView()

        contentView.setExpiry(nil)

        XCTAssertTrue(contentView.remainingTimeLabel.isHidden)
        XCTAssertEqual(
            contentView.accountExpiryRowView.accessibilityValue,
            NSLocalizedString("Currently unavailable", comment: "")
        )
    }

    func testAccountNumberRowExposesFullAddressAndShortDisplayForm() {
        let row = AccountNumberRow()
        let address = "wb7kgyLPQyPHzUsRPUyVBqLx5UGz9DnJ4vRfLMbShP9DnB"

        row.accountNumber = address

        // Copy and accessibility always carry the FULL address; only the
        // visible label is shortened.
        XCTAssertEqual(row.accessibilityValue, address)
    }
}
