package com.warrenbrowse.vpn.test.mockapi

import java.time.ZonedDateTime
import kotlin.time.Duration.Companion.minutes
import kotlinx.coroutines.test.runTest
import com.warrenbrowse.vpn.lib.model.Constraint
import com.warrenbrowse.vpn.lib.model.IpVersion
import com.warrenbrowse.vpn.lib.model.ObfuscationMode
import com.warrenbrowse.vpn.test.common.extension.acceptVpnPermissionDialog
import com.warrenbrowse.vpn.test.common.misc.RelayProvider
import com.warrenbrowse.vpn.test.common.page.ConnectPage
import com.warrenbrowse.vpn.test.common.page.SelectLocationPage
import com.warrenbrowse.vpn.test.common.page.enableServerIpOverrideStory
import com.warrenbrowse.vpn.test.common.page.on
import com.warrenbrowse.vpn.test.common.rule.ForgetAllVpnAppsInSettingsTestRule
import com.warrenbrowse.vpn.test.mockapi.constant.DEFAULT_DEVICE_LIST
import com.warrenbrowse.vpn.test.mockapi.constant.DUMMY_DEVICE_NAME_2
import com.warrenbrowse.vpn.test.mockapi.constant.DUMMY_ID_2
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.BeforeEach
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.extension.RegisterExtension

class ServerIpOverridesMockApiTest : MockApiTest() {

    @RegisterExtension
    @JvmField
    val forgetAllVpnAppsInSettingsTestRule = ForgetAllVpnAppsInSettingsTestRule()

    // We are using a static relay list in mock api tests that mirrors the production relay list, so
    // we should always check for production relays.
    private val relayProvider = RelayProvider("oss")
    private val validAccountNumber = "1234123412341234"

    @BeforeEach
    fun setupDispatcher() {
        apiRouter.apply {
            expectedAccountNumber = validAccountNumber
            accountExpiry = ZonedDateTime.now().plusMonths(1)
            devices = DEFAULT_DEVICE_LIST.toMutableMap()
            devicePendingToGetCreated = DUMMY_ID_2 to DUMMY_DEVICE_NAME_2
        }
    }

    @Test
    fun testAttemptToConnectUsingServerIpOverride() =
        runTest(timeout = 2.minutes) {
            // Arrange
            app.launchAndLogIn(validAccountNumber)
            app.applySettings(
                obfuscationMode = ObfuscationMode.Off,
                deviceIpVersion = Constraint.Only(IpVersion.IPV4),
            )

            // Enable server ip override
            val mockServerIp = "12.12.12.12"
            val relay = relayProvider.getOverrideRelay()
            on<ConnectPage> { enableServerIpOverrideStory(relay.relay, mockServerIp) }

            // Select the relay which has an overriden ip
            on<ConnectPage> { clickSelectLocation() }

            on<SelectLocationPage> { expandAndClickRelay(relay) }

            device.acceptVpnPermissionDialog()

            var inIpv4Address = ""

            on<ConnectPage> {
                waitForConnectingLabel()
                inIpv4Address = extractInIpAddress()
            }

            // Verify connection
            assertEquals(mockServerIp, inIpv4Address)
        }
}
