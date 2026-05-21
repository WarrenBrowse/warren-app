package com.warrenbrowse.vpn.test.common.page

import androidx.test.uiautomator.By
import com.warrenbrowse.vpn.lib.ui.tag.OUT_OF_TIME_SCREEN_TITLE_TEST_TAG
import com.warrenbrowse.vpn.test.common.extension.findObjectWithTimeout

class OutOfTimePage internal constructor() : Page() {
    private val outOfTimeSelector = By.res(OUT_OF_TIME_SCREEN_TITLE_TEST_TAG)

    override fun assertIsDisplayed() {
        uiDevice.findObjectWithTimeout(outOfTimeSelector)
    }

    fun clickAddTime() {
        uiDevice.findObjectWithTimeout(By.text("Add time")).click()
    }
}
