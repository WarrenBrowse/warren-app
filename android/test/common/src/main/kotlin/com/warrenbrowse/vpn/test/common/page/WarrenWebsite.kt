package com.warrenbrowse.vpn.test.common.page

import androidx.test.uiautomator.By
import com.warrenbrowse.vpn.test.common.constant.VERY_LONG_TIMEOUT
import com.warrenbrowse.vpn.test.common.extension.findObjectWithTimeout

class WarrenWebsite internal constructor() : Page() {
    override fun assertIsDisplayed() {
        uiDevice.findObjectWithTimeout(
            selector = By.text("Mullvad help center"),
            timeout = VERY_LONG_TIMEOUT,
        )
    }
}
