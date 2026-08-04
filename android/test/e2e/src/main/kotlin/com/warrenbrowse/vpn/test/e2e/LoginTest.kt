package com.warrenbrowse.vpn.test.e2e

import com.warrenbrowse.vpn.test.common.page.ConnectPage
import com.warrenbrowse.vpn.test.common.page.LoginPage
import com.warrenbrowse.vpn.test.common.page.on
import com.warrenbrowse.vpn.test.e2e.misc.AccountTestRule
import org.junit.jupiter.api.Disabled
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.extension.RegisterExtension

class LoginTest : EndToEndTest() {

    @RegisterExtension @JvmField val accountTestRule = AccountTestRule()

    @Test
    fun testLoginWithValidCredentials() {
        val validTestAccountNumber = accountTestRule.validAccountNumber

        app.launchAndEnsureOnLoginPage()

        on<LoginPage> {
            enterAccountNumber(validTestAccountNumber)
            clickLoginButton()
        }

        on<ConnectPage>()
    }

    @Test
    @Disabled("Failed login attempts are highly rate limited and cause test flakiness")
    fun testLoginWithInvalidCredentials() {
        val invalidDummyAccountNumber = accountTestRule.invalidAccountNumber

        app.launchAndEnsureOnLoginPage()

        on<LoginPage> {
            enterAccountNumber(invalidDummyAccountNumber)
            clickLoginButton()
            verifyShowingInvalidAccount()
        }
    }
}
