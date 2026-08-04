//
//  OutOfTimePage.swift
//  WarrenVPNUITests
//
//  Created by Niklas Berglund on 2024-02-20.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import Foundation
import XCTest

class OutOfTimePage: PaymentPage {
    @discardableResult override init(_ app: XCUIApplication) {
        super.init(app)

        self.pageElement = app.otherElements[.outOfTimeView]
        waitForPageToBeShown()
    }
}
