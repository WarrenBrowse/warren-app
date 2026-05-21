package com.warrenbrowse.vpn.test.common.page

import androidx.test.uiautomator.By
import com.warrenbrowse.vpn.test.common.extension.findObjectWithTimeout

class WireGuardCustomPortDialog internal constructor() : Page() {
    private val textFieldLabelSelector = By.text("Enter port")
    private val setPortSelector = By.text("Set port")
    private val cancelSelector = By.text("Cancel")

    override fun assertIsDisplayed() {
        uiDevice.findObjectWithTimeout(textFieldLabelSelector)
    }

    fun enterCustomPort(port: String) {
        uiDevice.findObjectWithTimeout(textFieldLabelSelector).parent.parent.text = port
    }

    fun clickSetPort() {
        uiDevice.findObjectWithTimeout(setPortSelector).click()
    }

    fun clickCancel() {
        uiDevice.findObjectWithTimeout(cancelSelector).click()
    }
}
