//
//  WarrenWalletDeviceStateTests.swift
//  WarrenVPNTests
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import WarrenSettings
import XCTest

final class WarrenWalletDeviceStateTests: XCTestCase {
    func test_walletBacked_carriesSS58AsAccountNumber() {
        let state = DeviceState.walletBacked(
            ss58Address: "wb7kgy8FF4exampleAddr",
            publicKeyHex: "00112233aabbccdd"
        )
        XCTAssertEqual(state.accountData?.number, "wb7kgy8FF4exampleAddr")
        XCTAssertEqual(state.accountData?.identifier, "00112233aabbccdd")
    }

    func test_walletBacked_isLoggedInAndNeverExpires() {
        let state = DeviceState.walletBacked(ss58Address: "wbAddr", publicKeyHex: "deadbeef")
        XCTAssertTrue(state.isLoggedIn)
        XCTAssertEqual(state.accountData?.expiry, .distantFuture)
        XCTAssertEqual(state.accountData?.isExpired, false)
    }

    func test_walletBacked_deviceDataIsPresentForScaffolding() {
        let state = DeviceState.walletBacked(ss58Address: "wbAddr", publicKeyHex: "deadbeef")
        XCTAssertEqual(state.deviceData?.identifier, "deadbeef")
        XCTAssertEqual(state.deviceData?.name, "warren-wallet")
    }
}
