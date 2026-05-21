package com.warrenbrowse.vpn.test.common.page

import androidx.test.uiautomator.By
import com.warrenbrowse.vpn.lib.ui.tag.SELECT_PORT_ITEM_X_TEST_TAG
import com.warrenbrowse.vpn.test.common.extension.clickObjectAwaitIsChecked
import com.warrenbrowse.vpn.test.common.extension.findObjectWithTimeout

class SelectPortPage internal constructor() : Page() {
    private val settingsSelector = By.text("Port")

    override fun assertIsDisplayed() {
        uiDevice.findObjectWithTimeout(settingsSelector)
    }

    fun clickPresetPort(port: Int) {
        uiDevice.clickObjectAwaitIsChecked(By.res(SELECT_PORT_ITEM_X_TEST_TAG.format(port)))
    }
}
