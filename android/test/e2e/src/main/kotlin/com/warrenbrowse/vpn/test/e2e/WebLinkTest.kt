package com.warrenbrowse.vpn.test.e2e

import com.warrenbrowse.vpn.test.common.page.LoginPage
import com.warrenbrowse.vpn.test.common.page.WarrenWebsite
import com.warrenbrowse.vpn.test.common.page.SettingsPage
import com.warrenbrowse.vpn.test.common.page.on
import org.junit.jupiter.api.Disabled
import org.junit.jupiter.api.Test

class WebLinkTest : EndToEndTest() {
    @Test
    @Disabled("Disabled due to broken in-browser text detection (DROID-2009)")
    fun testOpenFaqFromApp() {
        app.launchAndEnsureOnLoginPage()

        on<LoginPage> { clickSettings() }

        on<SettingsPage> { clickFaqAndGuides() }

        on<WarrenWebsite>()
    }
}
