package com.warrenbrowse.vpn.test.e2e

import androidx.test.uiautomator.By
import com.warrenbrowse.vpn.lib.ui.tag.CONNECT_CARD_HEADER_TEST_TAG
import com.warrenbrowse.vpn.test.common.annotation.SkipForFlavors
import com.warrenbrowse.vpn.test.common.constant.VERY_LONG_TIMEOUT
import com.warrenbrowse.vpn.test.common.extension.findObjectWithTimeout
import com.warrenbrowse.vpn.test.common.page.AddTimeBottomSheet
import com.warrenbrowse.vpn.test.common.page.LoginPage
import com.warrenbrowse.vpn.test.common.page.OutOfTimePage
import com.warrenbrowse.vpn.test.common.page.buyGooglePlayTime
import com.warrenbrowse.vpn.test.common.page.on
import com.warrenbrowse.vpn.test.e2e.annotations.RequiresGoogleBillingAccount
import com.warrenbrowse.vpn.test.e2e.annotations.RequiresPartnerAuth
import com.warrenbrowse.vpn.test.e2e.misc.AccountTestRule
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.extension.RegisterExtension

class PaymentTest : EndToEndTest() {

    @RegisterExtension @JvmField val accountTestRule = AccountTestRule(withTime = false)

    @Test
    @SkipForFlavors(currentFlavor = BuildConfig.FLAVOR_billing, "oss")
    @RequiresGoogleBillingAccount
    @RequiresPartnerAuth
    fun testInAppPurchaseForOutOfTime() {
        val validTestAccountNumber = accountTestRule.validAccountNumber

        app.launchAndEnsureOnLoginPage()

        on<LoginPage> {
            enterAccountNumber(validTestAccountNumber)
            clickLoginButton()
        }

        on<OutOfTimePage> { clickAddTime() }

        on<AddTimeBottomSheet> { click30days() }

        device.buyGooglePlayTime()

        // Assert we reach the Connect page after purchase
        device.findObjectWithTimeout(
            By.res(CONNECT_CARD_HEADER_TEST_TAG),
            timeout = VERY_LONG_TIMEOUT,
        )
    }
}
