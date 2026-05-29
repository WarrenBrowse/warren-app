package com.warrenbrowse.vpn.test.e2e

import android.net.InetAddresses.parseNumericAddress
import java.net.Inet6Address
import kotlin.time.Duration.Companion.milliseconds
import kotlin.time.Duration.Companion.minutes
import kotlinx.coroutines.delay
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.test.runTest
import com.warrenbrowse.vpn.lib.model.Constraint
import com.warrenbrowse.vpn.lib.model.IpVersion
import com.warrenbrowse.vpn.lib.model.ObfuscationMode
import com.warrenbrowse.vpn.test.common.constant.EXTREMELY_LONG_TIMEOUT
import com.warrenbrowse.vpn.test.common.extension.acceptVpnPermissionDialog
import com.warrenbrowse.vpn.test.common.interactor.DaitaOption
import com.warrenbrowse.vpn.test.common.misc.RelayProvider
import com.warrenbrowse.vpn.test.common.page.ConnectPage
import com.warrenbrowse.vpn.test.common.page.ObfuscationOption
import com.warrenbrowse.vpn.test.common.page.SelectLocationPage
import com.warrenbrowse.vpn.test.common.page.disablePostQuantumStory
import com.warrenbrowse.vpn.test.common.page.enableDeviceIpv6Story
import com.warrenbrowse.vpn.test.common.page.enableLocalNetworkSharingStory
import com.warrenbrowse.vpn.test.common.page.enableMultihopStory
import com.warrenbrowse.vpn.test.common.page.on
import com.warrenbrowse.vpn.test.common.page.setObfuscationStory
import com.warrenbrowse.vpn.test.common.page.toggleInTunnelIpv6Story
import com.warrenbrowse.vpn.test.common.rule.ForgetAllVpnAppsInSettingsTestRule
import com.warrenbrowse.vpn.test.e2e.annotations.HasDependencyOnLocalAPI
import com.warrenbrowse.vpn.test.e2e.api.connectioncheck.ConnectionCheckApi
import com.warrenbrowse.vpn.test.e2e.api.relay.RelayApi
import com.warrenbrowse.vpn.test.e2e.misc.AccountTestRule
import com.warrenbrowse.vpn.test.e2e.misc.ClearFirewallRules
import com.warrenbrowse.vpn.test.e2e.router.firewall.DropRule
import com.warrenbrowse.vpn.test.e2e.router.firewall.FirewallClient
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Disabled
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.assertInstanceOf
import org.junit.jupiter.api.extension.RegisterExtension

class ConnectionTest : EndToEndTest() {

    @RegisterExtension @JvmField val accountTestRule = AccountTestRule()

    @RegisterExtension
    @JvmField
    val forgetAllVpnAppsInSettingsTestRule = ForgetAllVpnAppsInSettingsTestRule()

    private val connCheckClient = ConnectionCheckApi()
    private val relayClient = RelayApi()
    private val firewallClient by lazy { FirewallClient() }
    private val relayProvider = RelayProvider()

    @Test
    fun testConnect() {
        // Given
        app.launchAndLogIn(accountTestRule.validAccountNumber)

        on<ConnectPage> { clickConnect() }

        device.acceptVpnPermissionDialog()

        on<ConnectPage> { waitForConnectedLabel() }
    }

    @Test
    fun testConnectAndVerifyWithConnectionCheck() = runTest {
        // Given
        app.launchAndLogIn(accountTestRule.validAccountNumber)

        on<ConnectPage> { clickConnect() }

        device.acceptVpnPermissionDialog()

        var outIpv4Address = ""

        on<ConnectPage> {
            waitForConnectedLabel()
            outIpv4Address = extractOutIpv4Address()
        }

        // Then
        val result = connCheckClient.connectionCheck()

        assertEquals(result.ip, outIpv4Address)
    }

    @Test
    fun testConnectingWithoutPostQuantum() = runTest {
        // Given
        app.launchAndLogIn(accountTestRule.validAccountNumber)

        on<ConnectPage> { disablePostQuantumStory() }

        // Connect
        on<ConnectPage> { clickConnect() }

        device.acceptVpnPermissionDialog()

        var outIpv4Address = ""

        on<ConnectPage> {
            waitForConnectedLabel()
            outIpv4Address = extractOutIpv4Address()
        }

        val result = connCheckClient.connectionCheck()

        // Verify connection
        assertEquals(result.ip, outIpv4Address)
    }

    @Test
    @HasDependencyOnLocalAPI
    @ClearFirewallRules
    fun testWireGuardObfuscationAutomatic() =
        runTest(timeout = 2.minutes) {
            app.launchAndLogIn(accountTestRule.validAccountNumber)
            on<ConnectPage> { enableLocalNetworkSharingStory() }

            on<ConnectPage> { clickSelectLocation() }

            on<SelectLocationPage> {
                clickLocationExpandButton(relayProvider.getDefaultRelay().country)
                clickLocationExpandButton(relayProvider.getDefaultRelay().city)
                clickLocationCell(relayProvider.getDefaultRelay().relay)
            }

            device.acceptVpnPermissionDialog()

            var relayIpAddress: String? = null

            on<ConnectPage> {
                waitForConnectedLabel()
                relayIpAddress = extractInIpAddress()
                clickDisconnect()
            }

            // Block UDP traffic to the relay
            createFirewallRules { DropRule.blockUDPTrafficRule(relayIpAddress!!) }

            on<ConnectPage> {
                clickConnect()
                // Currently it takes ~60 seconds to connect with wg obfuscation automatic and UDP
                // traffic blocked so we need to be very forgiving
                // The order of obfuscation methods in automatic mode can be found here:
                // mullvad-relay-selector/src/relay_selector/mod.rs
                waitForConnectedLabel(timeout = VERY_FORGIVING_WIREGUARD_OFF_CONNECTION_TIMEOUT)
            }
        }

    @Test
    @HasDependencyOnLocalAPI
    @ClearFirewallRules
    fun testWireGuardObfuscationOff() =
        runTest(timeout = 2.minutes) {
            app.launchAndLogIn(accountTestRule.validAccountNumber)
            app.applySettings(localNetworkSharing = true)

            on<ConnectPage> { clickSelectLocation() }

            on<SelectLocationPage> {
                clickLocationExpandButton(relayProvider.getDefaultRelay().country)
                clickLocationExpandButton(relayProvider.getDefaultRelay().city)
                clickLocationCell(relayProvider.getDefaultRelay().relay)
            }

            device.acceptVpnPermissionDialog()

            var relayIpAddress: String? = null

            on<ConnectPage> {
                waitForConnectedLabel()
                relayIpAddress = extractInIpAddress()
                clickDisconnect()
            }

            // Block UDP traffic to the relay
            createFirewallRules { DropRule.blockUDPTrafficRule(relayIpAddress!!) }

            app.applySettings(
                obfuscationMode = ObfuscationMode.Off,
                deviceIpVersion = Constraint.Only(IpVersion.IPV4),
            )

            on<ConnectPage> {
                clickConnect()
                // Ensure it is not possible to connect to relay.
                // Give it some time and then verify still unable to connect.
                // This duration must be long enough to ensure all retry attempts have been made.
                runBlocking { delay(UNSUCCESSFUL_CONNECTION_TIMEOUT.milliseconds) }
                waitForConnectingLabel()
                clickCancel()
            }
        }

    @Test
    fun testDaita() =
        runTest(timeout = 2.minutes) {
            app.launchAndLogIn(accountTestRule.validAccountNumber)
            app.applySettings(daita = DaitaOption.Auto(true))

            on<ConnectPage> { clickSelectLocation() }

            on<SelectLocationPage> {
                clickLocationExpandButton(relayProvider.getNonDaitaRelay().country)
                clickLocationExpandButton(relayProvider.getNonDaitaRelay().city)
                clickLocationCell(relayProvider.getNonDaitaRelay().relay)
            }

            device.acceptVpnPermissionDialog()

            on<ConnectPage> {
                waitForConnectedLabel()
                app.applySettings(daita = DaitaOption.DirectOnly(true))
                waitForBlockedLabel()
                clickSelectLocation()
            }

            on<SelectLocationPage> {
                assertDaitaChipVisible()
                clickLocationExpandButton(relayProvider.getDaitaRelay().country)
                clickLocationExpandButton(relayProvider.getDaitaRelay().city)
                clickLocationCell(relayProvider.getDaitaRelay().relay)
            }

            on<ConnectPage> { waitForConnectedLabel() }
        }

    @Test
    @HasDependencyOnLocalAPI
    @ClearFirewallRules
    fun testUDPOverTCP() = runTest {
        app.launchAndLogIn(accountTestRule.validAccountNumber)
        app.applySettings(localNetworkSharing = true)

        on<ConnectPage> { clickSelectLocation() }

        on<SelectLocationPage> {
            clickLocationExpandButton(relayProvider.getDefaultRelay().country)
            clickLocationExpandButton(relayProvider.getDefaultRelay().city)
            clickLocationCell(relayProvider.getDefaultRelay().relay)
        }

        device.acceptVpnPermissionDialog()

        var relayIpAddress: String? = null

        on<ConnectPage> {
            waitForConnectedLabel()
            relayIpAddress = extractInIpAddress()
            clickDisconnect()
        }

        // Block UDP traffic to the relay
        createFirewallRules { DropRule.blockUDPTrafficRule(relayIpAddress!!) }

        // Enable UDP-over-TCP
        on<ConnectPage> { setObfuscationStory(ObfuscationOption.Udp2Tcp) }

        on<ConnectPage> {
            clickConnect()
            waitForConnectedLabel(timeout = EXTREMELY_LONG_TIMEOUT)
            clickDisconnect()
        }
    }

    @Test
    @HasDependencyOnLocalAPI
    @ClearFirewallRules
    fun testQuic() = runTest {
        app.launchAndLogIn(accountTestRule.validAccountNumber)
        app.applySettings(localNetworkSharing = true)

        on<ConnectPage> { clickSelectLocation() }

        on<SelectLocationPage> {
            val quicRelay = relayProvider.getQuicRelay()
            clickLocationExpandButton(quicRelay.country)
            clickLocationExpandButton(quicRelay.city)
            scrollUntilCell(quicRelay.relay)
            clickLocationCell(quicRelay.relay)
        }

        device.acceptVpnPermissionDialog()

        var relayIpAddress: String? = null

        on<ConnectPage> {
            waitForConnectedLabel()
            relayIpAddress = extractInIpAddress()
            clickDisconnect()
        }

        // Block UDP traffic to the relay
        createFirewallRules { DropRule.blockWireGuardTrafficRule(relayIpAddress!!) }

        // Enable QUIC
        on<ConnectPage> { setObfuscationStory(ObfuscationOption.Quic) }

        on<ConnectPage> {
            clickConnect()
            waitForConnectedLabel(timeout = EXTREMELY_LONG_TIMEOUT)
            clickDisconnect()
        }
    }

    @Test
    @HasDependencyOnLocalAPI
    @ClearFirewallRules
    fun testLwo() = runTest {
        app.launchAndLogIn(accountTestRule.validAccountNumber)

        app.applySettings(localNetworkSharing = true)

        on<ConnectPage> { clickSelectLocation() }

        on<SelectLocationPage> {
            val lwoRelay = relayProvider.getLwoRelay()
            clickLocationExpandButton(lwoRelay.country)
            clickLocationExpandButton(lwoRelay.city)
            scrollUntilCell(lwoRelay.relay)
            clickLocationCell(lwoRelay.relay)
        }

        device.acceptVpnPermissionDialog()

        var relayIpAddress: String? = null

        on<ConnectPage> {
            waitForConnectedLabel()
            relayIpAddress = extractInIpAddress()
            clickDisconnect()
        }

        // Block UDP traffic to the relay
        createFirewallRules { DropRule.blockWireGuardTrafficRule(relayIpAddress!!) }

        // Enable LWO
        on<ConnectPage> { setObfuscationStory(ObfuscationOption.Lwo) }

        on<ConnectPage> {
            clickConnect()
            waitForConnectedLabel(timeout = EXTREMELY_LONG_TIMEOUT)
            clickDisconnect()
        }
    }

    @Test
    @HasDependencyOnLocalAPI
    @ClearFirewallRules
    fun testShadowsocks() =
        runTest(timeout = 2.minutes) {
            app.launchAndLogIn(accountTestRule.validAccountNumber)
            app.applySettings(localNetworkSharing = true, obfuscationMode = ObfuscationMode.Off)

            // Block all WireGuard traffic
            createFirewallRules { DropRule.blockWireGuardTrafficRule(ANY_IPV4_ADDRESS) }

            on<ConnectPage> { clickConnect() }

            device.acceptVpnPermissionDialog()

            // Ensure it is not possible to connect to relay
            on<ConnectPage> {
                runBlocking { delay(UNSUCCESSFUL_CONNECTION_TIMEOUT.milliseconds) }
                waitForConnectingLabel()
                clickCancel()
            }

            on<ConnectPage> { setObfuscationStory(ObfuscationOption.Shadowsocks) }

            // Ensure we can now connect with Shadowsocks enabled
            on<ConnectPage> {
                clickConnect()
                waitForConnectedLabel(timeout = EXTREMELY_LONG_TIMEOUT)
                clickDisconnect()
            }
        }

    @Test
    @HasDependencyOnLocalAPI
    @ClearFirewallRules
    fun testApiUnavailable() = runTest {
        val testRelayIp = relayClient.getDefaultRelayIpAddress()

        app.launchAndLogIn(accountTestRule.validAccountNumber)
        on<ConnectPage>()

        // Block everything except the default relay IP. After this the API is no longer reachable.
        createFirewallRules { DropRule.blockAllTrafficExceptToDestinationRule(testRelayIp) }

        // Restarting the activity will re-create the daemon which will try to reach the API.
        targetActivity.finishAffinity()
        app.launch()

        on<ConnectPage> { clickSelectLocation() }

        on<SelectLocationPage> {
            clickLocationExpandButton(relayProvider.getDefaultRelay().country)
            clickLocationExpandButton(relayProvider.getDefaultRelay().city)
            clickLocationCell(relayProvider.getDefaultRelay().relay)
        }

        device.acceptVpnPermissionDialog()

        // Test that we can still connect to the relay even though the API is blocked.
        on<ConnectPage> {
            waitForConnectedLabel()
            clickDisconnect()
            waitForDisconnectedLabel()
        }
    }

    @Test
    fun testConnectWithoutInTunnelIpv6() = runTest {
        // Given
        app.launchAndLogIn(accountTestRule.validAccountNumber)

        on<ConnectPage> { toggleInTunnelIpv6Story() }
        on<ConnectPage> { clickConnect() }
        device.acceptVpnPermissionDialog()

        on<ConnectPage> { waitForConnectedLabel() }
    }

    @Test
    fun testConnectUsingMultihop() = runTest {
        // Given
        app.launchAndLogIn(accountTestRule.validAccountNumber)

        // Enable multihop
        on<ConnectPage> { enableMultihopStory() }

        // Select default relay as out relay
        on<ConnectPage> { clickSelectLocation() }
        val defaultRelay = relayProvider.getDefaultRelay()
        on<SelectLocationPage> {
            clickLocationExpandButton(defaultRelay.country)
            clickLocationExpandButton(defaultRelay.city)
            scrollUntilCell(defaultRelay.relay)
            clickLocationCell(defaultRelay.relay)
        }

        device.acceptVpnPermissionDialog()

        var outIpv4Address = ""
        on<ConnectPage> {
            waitForConnectedLabel()
            outIpv4Address = extractOutIpv4Address()
        }

        val result = connCheckClient.connectionCheck()

        // Check IPs match and that the out server is default server
        assertEquals(result.ip, outIpv4Address)
        assertEquals(result.mullvadExitIpHostname, defaultRelay.relay)
    }

    @Test
    @Disabled(
        "Disabled due to IPv6 will occasionally stop working on Android phones due to a system bug. "
    )
    fun testConnectUsingIpv6() = runTest {
        // Given
        app.launchAndLogIn(accountTestRule.validAccountNumber)

        // Set Device IP version
        on<ConnectPage> { enableDeviceIpv6Story() }

        // Connect
        on<ConnectPage> { clickConnect() }

        device.acceptVpnPermissionDialog()

        var inIpv6Address = ""

        on<ConnectPage> {
            waitForConnectedLabel()
            inIpv6Address = extractInIpAddress()
        }

        val parsedAddress = parseNumericAddress(inIpv6Address)
        // Verify that the in address is a IPv6 address
        assertInstanceOf<Inet6Address>(parsedAddress)
    }

    private suspend fun createFirewallRules(block: () -> List<DropRule>) =
        block().forEach { firewallClient.createRule(it) }

    companion object {
        const val VERY_FORGIVING_WIREGUARD_OFF_CONNECTION_TIMEOUT = 80000L
        const val UNSUCCESSFUL_CONNECTION_TIMEOUT = 30000L
        const val ANY_IPV4_ADDRESS = "0.0.0.0/0"
    }
}
