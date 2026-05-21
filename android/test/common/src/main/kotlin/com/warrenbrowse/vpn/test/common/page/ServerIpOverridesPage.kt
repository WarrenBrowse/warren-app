package com.warrenbrowse.vpn.test.common.page

import androidx.test.uiautomator.By
import com.warrenbrowse.vpn.lib.ui.tag.SERVER_IP_OVERRIDE_IMPORT_TEST_TAG
import com.warrenbrowse.vpn.test.common.extension.findObjectWithTimeout

class ServerIpOverridesPage internal constructor() : Page() {
    private val serverIpOverrideSelector = By.text("Server IP override")

    override fun assertIsDisplayed() {
        uiDevice.findObjectWithTimeout(serverIpOverrideSelector)
    }

    fun clickImportButton() {
        uiDevice.findObjectWithTimeout(By.res(SERVER_IP_OVERRIDE_IMPORT_TEST_TAG)).click()
    }

    fun assertOverrideActive() {
        uiDevice.findObjectWithTimeout(By.text("Overrides active"))
    }
}
