//
//  WarrenWalletRoutingTests.swift
//  WarrenVPNTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import XCTest

@testable import WarrenVPN

final class WarrenWalletRoutingTests: XCTestCase {
    func test_termsOfServiceGatesEverything() {
        XCTAssertEqual(
            nextWarrenRoutes(
                updateBlocked: false, agreedToTOS: false, onboardingComplete: true,
                isRevoked: false, walletExists: true, isExpired: false),
            [.tos]
        )
    }

    func test_noWalletRoutesToLoginBeforeOnboarding() {
        // Fresh install: the identity comes first, the wizard follows
        // (desktop parity: LoginView, then the onboarding routes).
        XCTAssertEqual(
            nextWarrenRoutes(
                updateBlocked: false, agreedToTOS: true, onboardingComplete: false,
                isRevoked: false, walletExists: false, isExpired: false),
            [.login]
        )
    }

    func test_walletWithoutOnboardingRoutesToWizard() {
        XCTAssertEqual(
            nextWarrenRoutes(
                updateBlocked: false, agreedToTOS: true, onboardingComplete: false,
                isRevoked: false, walletExists: true, isExpired: false),
            [.warrenOnboarding]
        )
    }

    func test_revokedDeviceWinsOverWalletPresence() {
        XCTAssertEqual(
            nextWarrenRoutes(
                updateBlocked: false, agreedToTOS: true, onboardingComplete: true,
                isRevoked: true, walletExists: true, isExpired: false),
            [.revoked]
        )
    }

    func test_noWalletRoutesToLogin() {
        XCTAssertEqual(
            nextWarrenRoutes(
                updateBlocked: false, agreedToTOS: true, onboardingComplete: true,
                isRevoked: false, walletExists: false, isExpired: false),
            [.login]
        )
    }

    func test_walletPresentRoutesToMain() {
        XCTAssertEqual(
            nextWarrenRoutes(
                updateBlocked: false, agreedToTOS: true, onboardingComplete: true,
                isRevoked: false, walletExists: true, isExpired: false),
            [.main]
        )
    }

    func test_expiredSubscriptionGatesMainBehindOutOfTime() {
        XCTAssertEqual(
            nextWarrenRoutes(
                updateBlocked: false, agreedToTOS: true, onboardingComplete: true,
                isRevoked: false, walletExists: true, isExpired: true),
            [.main, .outOfTime]
        )
    }

    func test_expiredDuringOnboardingStaysInWizard() {
        // A fresh wallet is always unfunded; the wizard's subscription
        // step is its funding flow, so the gate must not preempt it.
        XCTAssertEqual(
            nextWarrenRoutes(
                updateBlocked: false, agreedToTOS: true, onboardingComplete: false,
                isRevoked: false, walletExists: true, isExpired: true),
            [.warrenOnboarding]
        )
    }
}
