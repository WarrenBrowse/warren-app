package com.warrenbrowse.vpn.test.common.page

import androidx.test.uiautomator.By
import com.warrenbrowse.vpn.test.common.extension.findObjectWithTimeout

class PrivacyPage internal constructor() : Page() {
    private val privacySelector = By.text("Privacy")
    private val agreeSelector = By.text("Agree and continue")

    override fun assertIsDisplayed() {
        uiDevice.findObjectWithTimeout(privacySelector)
    }

    fun clickAgreeOnPrivacyDisclaimer() {
        uiDevice.findObjectWithTimeout(agreeSelector).click()
    }
}
