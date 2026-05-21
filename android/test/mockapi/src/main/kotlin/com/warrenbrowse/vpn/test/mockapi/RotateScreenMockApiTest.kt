package com.warrenbrowse.vpn.test.mockapi

import androidx.test.uiautomator.waitForStableInActiveWindow
import java.time.ZonedDateTime
import com.warrenbrowse.vpn.test.common.page.ConnectPage
import com.warrenbrowse.vpn.test.common.page.on
import com.warrenbrowse.vpn.test.mockapi.constant.ALMOST_FULL_DEVICE_LIST
import com.warrenbrowse.vpn.test.mockapi.constant.DUMMY_DEVICE_NAME_1
import com.warrenbrowse.vpn.test.mockapi.constant.DUMMY_ID_1
import org.junit.jupiter.api.Test

class RotateScreenMockApiTest : MockApiTest() {
    @Test
    fun testRotatingTheDeviceDoesNotCrashTheApp() {
        val validAccountNumber = "1234123412341234"
        apiRouter.apply {
            expectedAccountNumber = validAccountNumber
            accountExpiry = ZonedDateTime.now().plusMonths(1)
            devices = ALMOST_FULL_DEVICE_LIST.toMutableMap()
            devicePendingToGetCreated = DUMMY_ID_1 to DUMMY_DEVICE_NAME_1
        }

        app.launchAndLogIn(validAccountNumber)

        on<ConnectPage> {
            device.waitForStableInActiveWindow()
            device.setOrientationLeft()
            device.waitForStableInActiveWindow()
            device.unfreezeRotation()
            device.setOrientationNatural()
            device.waitForStableInActiveWindow()
            device.unfreezeRotation()
            device.setOrientationRight()
            device.waitForStableInActiveWindow()
            device.unfreezeRotation()
            device.setOrientationNatural()
        }

        on<ConnectPage> {
            // Make sure the connect screen is showing as expected.
        }
    }
}
