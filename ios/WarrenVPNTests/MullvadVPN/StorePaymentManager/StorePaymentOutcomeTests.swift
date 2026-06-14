//
//  StorePaymentOutcomeTests.swift
//  WarrenVPNTests
//
//  Created by Warren on 2026-06-14.
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  Covers the `StorePaymentOutcome` value logic: the `timeAdded`
//  accessor, the `.day`-formatted string, and the context-keyed alert
//  message branching. The alert copy is the user-facing confirmation
//  after a wallet credit, so the noTimeAdded / timeAdded branch under
//  `.restoration` must not collapse.
//

import XCTest
@testable import WarrenVPN

final class StorePaymentOutcomeTests: XCTestCase {
    func test_timeAdded_isZero_forNoTimeAdded() {
        XCTAssertEqual(StorePaymentOutcome.noTimeAdded.timeAdded, 0)
    }

    func test_timeAdded_returnsAssociatedValue() {
        let oneDay: TimeInterval = 86_400
        XCTAssertEqual(StorePaymentOutcome.timeAdded(oneDay).timeAdded, oneDay)
    }

    func test_formattedTimeAdded_rendersDays() {
        let thirtyDays: TimeInterval = 30 * 86_400
        let formatted = StorePaymentOutcome.timeAdded(thirtyDays).formattedTimeAdded
        XCTAssertNotNil(formatted)
        XCTAssertTrue(
            formatted?.contains("30") == true,
            "Expected a 30-day rendering, got: \(formatted ?? "nil")"
        )
    }

    func test_alertMessage_purchase_embedsFormattedTime() {
        let outcome = StorePaymentOutcome.timeAdded(30 * 86_400)
        let message = outcome.alertMessage(for: .purchase)
        XCTAssertTrue(message.contains("30"), "Purchase alert should embed the added time, got: \(message)")
    }

    func test_alertMessage_restoration_distinguishesNoTimeFromTimeAdded() {
        let noTime = StorePaymentOutcome.noTimeAdded.alertMessage(for: .restoration)
        let withTime = StorePaymentOutcome.timeAdded(30 * 86_400).alertMessage(for: .restoration)
        XCTAssertFalse(noTime.isEmpty)
        XCTAssertFalse(withTime.isEmpty)
        XCTAssertNotEqual(
            noTime,
            withTime,
            "restoration alert must read differently when nothing was credited vs when time was added"
        )
    }

    func test_context_alertTitle_isNonEmpty_forEveryContext() {
        let contexts: [StorePaymentOutcome.Context] = [.purchase, .restoration, .restorationBeforePurchase]
        for context in contexts {
            XCTAssertFalse(context.alertTitle.isEmpty, "alertTitle empty for \(context)")
            XCTAssertFalse(context.errorTitle.isEmpty, "errorTitle empty for \(context)")
        }
    }
}
