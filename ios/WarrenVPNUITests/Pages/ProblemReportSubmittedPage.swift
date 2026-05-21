//
//  ProblemReportSubmittedPage.swift
//  WarrenVPNUITests
//
//  Created by Niklas Berglund on 2024-02-26.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import Foundation
import XCTest

class ProblemReportSubmittedPage: Page {
    @discardableResult override init(_ app: XCUIApplication) {
        super.init(app)

        self.pageElement = app.otherElements[.problemReportSubmittedView]
        waitForPageToBeShown()
    }
}
