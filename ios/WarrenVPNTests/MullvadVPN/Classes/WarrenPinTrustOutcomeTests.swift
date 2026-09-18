//
//  WarrenPinTrustOutcomeTests.swift
//  WarrenVPNTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import XCTest

@testable import WarrenVPN

/// What the user is told after trusting an exit's new key. Android decides
/// the same four ways in its own trust handler.
final class WarrenPinTrustOutcomeTests: XCTestCase {
    private let trusted = String(repeating: "ab", count: 32)
    private let other = String(repeating: "cd", count: 32)

    func testAReconnectThatTookSaysNothing() {
        let outcome = WarrenPinTrust.outcome(
            saved: true, observedAgain: nil, trusted: trusted,
            exitStillKnown: true, reconnected: true)

        XCTAssertEqual(outcome, .dispatched)
        XCTAssertNil(WarrenPinTrust.message(for: outcome))
    }

    func testAPinThatCouldNotBeWrittenOutranksEverything() {
        XCTAssertEqual(
            WarrenPinTrust.outcome(
                saved: false, observedAgain: other, trusted: trusted,
                exitStillKnown: false, reconnected: false),
            .saveFailed)
    }

    /// A key that changes a second time between the question and the answer
    /// is the shape an attack takes, and it used to come back as the same
    /// alert with no hint that anything had happened twice.
    func testASecondKeyIsNamedAsSuchRatherThanRepeatingTheFirstAlert() {
        XCTAssertEqual(
            WarrenPinTrust.outcome(
                saved: true, observedAgain: other, trusted: trusted,
                exitStillKnown: true, reconnected: false),
            .keyChangedAgain)
    }

    /// The finding holds whether or not the reconnect went out afterwards:
    /// the exit served a key nobody agreed to.
    func testASecondKeyOutranksAReconnectThatTook() {
        XCTAssertEqual(
            WarrenPinTrust.outcome(
                saved: true, observedAgain: other, trusted: trusted,
                exitStillKnown: true, reconnected: true),
            .keyChangedAgain)
    }

    /// The same key coming back is the reconnect catching up, not a change.
    func testTheSameKeyBackIsNotASecondChange() {
        for echoed in [trusted, trusted.uppercased()] {
            XCTAssertEqual(
                WarrenPinTrust.outcome(
                    saved: true, observedAgain: echoed, trusted: trusted,
                    exitStillKnown: true, reconnected: true),
                .dispatched,
                echoed)
        }
    }

    /// An exit that has left the roster is an ordinary fleet change, and
    /// calling it a failed trust sends the user looking for a security
    /// problem in a server that was simply retired.
    func testAnExitThatLeftTheRosterIsNamedApartFromAFailedTrust() {
        XCTAssertEqual(
            WarrenPinTrust.outcome(
                saved: true, observedAgain: nil, trusted: trusted,
                exitStillKnown: false, reconnected: false),
            .exitGone)
        XCTAssertEqual(
            WarrenPinTrust.outcome(
                saved: true, observedAgain: nil, trusted: trusted,
                exitStillKnown: true, reconnected: false),
            .trustFailed)
    }

    func testEveryOutcomeButASuccessCarriesItsOwnWording() {
        let messages = [
            WarrenPinTrustOutcome.saveFailed, .keyChangedAgain, .exitGone, .trustFailed,
        ].map { WarrenPinTrust.message(for: $0) }

        XCTAssertFalse(messages.contains(nil))
        XCTAssertEqual(Set(messages.compactMap { $0 }).count, messages.count, "two outcomes share a line")
    }
}
