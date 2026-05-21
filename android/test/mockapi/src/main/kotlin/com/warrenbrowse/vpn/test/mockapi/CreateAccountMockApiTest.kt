package com.warrenbrowse.vpn.test.mockapi

import androidx.test.uiautomator.By
import com.warrenbrowse.vpn.test.common.extension.findObjectWithTimeout
import com.warrenbrowse.vpn.test.common.page.LoginPage
import com.warrenbrowse.vpn.test.common.page.WelcomePage
import com.warrenbrowse.vpn.test.common.page.dismissStorePasswordPromptIfShown
import com.warrenbrowse.vpn.test.common.page.on
import com.warrenbrowse.vpn.test.mockapi.constant.DUMMY_DEVICE_NAME_2
import com.warrenbrowse.vpn.test.mockapi.constant.DUMMY_ID_2
import org.junit.jupiter.api.Test

class CreateAccountMockApiTest : MockApiTest() {
    @Test
    fun testCreateAccountSuccessful() {
        // Arrange
        val createdAccountNumber = "1234123412341234"
        apiRouter.apply {
            expectedAccountNumber = createdAccountNumber
            devicePendingToGetCreated = DUMMY_ID_2 to DUMMY_DEVICE_NAME_2
        }
        app.launchAndEnsureOnLoginPage()

        on<LoginPage> { clickCreateAccount() }

        device.dismissStorePasswordPromptIfShown()

        on<WelcomePage> {
            // Ensure account number is visible
            device.findObjectWithTimeout(By.text("1234 1234 1234 1234"))
        }
    }

    @Test
    fun testCreateAccountFailed() {
        // Arrange
        app.launchAndEnsureOnLoginPage()

        on<LoginPage> {
            clickCreateAccount()
            device.findObjectWithTimeout(By.text("Failed to create account"))
        }
    }
}
