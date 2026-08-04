package com.warrenbrowse.vpn.test.common.page

import androidx.test.uiautomator.By
import com.warrenbrowse.vpn.lib.ui.tag.DAITA_SCREEN_TEST_TAG
import com.warrenbrowse.vpn.lib.ui.tag.SWITCH_TEST_TAG
import com.warrenbrowse.vpn.test.common.extension.findObjectWithTimeout

class DaitaSettingsPage internal constructor() : Page() {
    private val enableSelector = By.text("Enable")

    override fun assertIsDisplayed() {
        uiDevice.findObjectWithTimeout(By.res(DAITA_SCREEN_TEST_TAG))
    }

    fun clickEnableSwitch() {
        val localNetworkSharingCell = uiDevice.findObjectWithTimeout(enableSelector).parent
        val localNetworkSharingSwitch =
            localNetworkSharingCell.findObjectWithTimeout(By.res(SWITCH_TEST_TAG))

        localNetworkSharingSwitch.click()
    }
}
