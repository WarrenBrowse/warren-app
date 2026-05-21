package com.warrenbrowse.vpn.test.mockapi

import java.time.ZonedDateTime
import com.warrenbrowse.vpn.test.common.page.ConnectPage
import com.warrenbrowse.vpn.test.common.page.TooManyDevicesPage
import com.warrenbrowse.vpn.test.common.page.on
import com.warrenbrowse.vpn.test.common.page.removeDeviceFlow
import com.warrenbrowse.vpn.test.mockapi.constant.DUMMY_DEVICE_NAME_6
import com.warrenbrowse.vpn.test.mockapi.constant.DUMMY_ID_6
import com.warrenbrowse.vpn.test.mockapi.constant.FULL_DEVICE_LIST
import org.junit.jupiter.api.Test

class TooManyDevicesMockApiTest : MockApiTest() {
    @Test
    fun testRemoveDeviceSuccessfulAndLogin() {
        // Arrange
        val validAccountNumber = "1234123412341234"
        apiRouter.apply {
            expectedAccountNumber = validAccountNumber
            accountExpiry = ZonedDateTime.now().plusMonths(1)
            devices = FULL_DEVICE_LIST.toMutableMap()
            devicePendingToGetCreated = DUMMY_ID_6 to DUMMY_DEVICE_NAME_6
        }

        // Act
        app.launchAndLogIn(validAccountNumber)

        // Assert that we have too many devices
        on<TooManyDevicesPage> {
            removeDeviceFlow(FULL_DEVICE_LIST.values.first())

            assertReadyToLogin()
            clickContinueWithLogin()
        }

        on<ConnectPage>()
    }
}
