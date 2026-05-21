package com.warrenbrowse.vpn.test.e2e

import com.warrenbrowse.vpn.test.common.page.AccountPage
import com.warrenbrowse.vpn.test.common.page.ConnectPage
import com.warrenbrowse.vpn.test.common.page.LoginPage
import com.warrenbrowse.vpn.test.common.page.on
import com.warrenbrowse.vpn.test.e2e.misc.AccountTestRule
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.extension.RegisterExtension

class LogoutTest : EndToEndTest() {

    @RegisterExtension @JvmField val accountTestRule = AccountTestRule()

    @Test
    fun testLogout() {
        // Given
        app.launchAndLogIn(accountTestRule.validAccountNumber)

        on<ConnectPage> { clickAccount() }

        on<AccountPage> { clickLogOut() }

        on<LoginPage>()
    }
}
