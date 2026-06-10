//
//  WarrenWalletRoutingTests.swift
//  WarrenVPNTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import XCTest


final class WarrenWalletRoutingTests: XCTestCase {
    func test_termsOfServiceGatesEverything() {
        XCTAssertEqual(
            nextWarrenRoutes(agreedToTOS: false, onboardingComplete: true, isRevoked: false, walletExists: true),
            [.tos]
        )
    }

    func test_onboardingWizardRunsOnceBeforeLogin() {
        XCTAssertEqual(
            nextWarrenRoutes(agreedToTOS: true, onboardingComplete: false, isRevoked: false, walletExists: false),
            [.warrenOnboarding]
        )
    }

    func test_revokedDeviceWinsOverWalletPresence() {
        XCTAssertEqual(
            nextWarrenRoutes(agreedToTOS: true, onboardingComplete: true, isRevoked: true, walletExists: true),
            [.revoked]
        )
    }

    func test_noWalletRoutesToLogin() {
        XCTAssertEqual(
            nextWarrenRoutes(agreedToTOS: true, onboardingComplete: true, isRevoked: false, walletExists: false),
            [.login]
        )
    }

    func test_walletPresentRoutesToMain() {
        XCTAssertEqual(
            nextWarrenRoutes(agreedToTOS: true, onboardingComplete: true, isRevoked: false, walletExists: true),
            [.main]
        )
    }
}
