package com.warrenbrowse.vpn.test.e2e

import androidx.test.platform.app.InstrumentationRegistry
import kotlin.time.Duration.Companion.milliseconds
import kotlin.time.Duration.Companion.minutes
import kotlinx.coroutines.delay
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.test.runTest
import com.warrenbrowse.vpn.lib.model.Constraint
import com.warrenbrowse.vpn.lib.model.ObfuscationMode
import com.warrenbrowse.vpn.lib.model.Port
import com.warrenbrowse.vpn.lib.model.QuantumResistantState
import com.warrenbrowse.vpn.test.common.extension.acceptVpnPermissionDialog
import com.warrenbrowse.vpn.test.common.misc.Attachment
import com.warrenbrowse.vpn.test.common.misc.RelayProvider
import com.warrenbrowse.vpn.test.common.page.ConnectPage
import com.warrenbrowse.vpn.test.common.page.SelectLocationPage
import com.warrenbrowse.vpn.test.common.page.enableDAITAStory
import com.warrenbrowse.vpn.test.common.page.on
import com.warrenbrowse.vpn.test.common.rule.ForgetAllVpnAppsInSettingsTestRule
import com.warrenbrowse.vpn.test.e2e.annotations.HasDependencyOnLocalAPI
import com.warrenbrowse.vpn.test.e2e.constant.getTrafficGeneratorHost
import com.warrenbrowse.vpn.test.e2e.constant.getTrafficGeneratorPort
import com.warrenbrowse.vpn.test.e2e.misc.AccountTestRule
import com.warrenbrowse.vpn.test.e2e.misc.NetworkTrafficChecker
import com.warrenbrowse.vpn.test.e2e.misc.NoTrafficToHostRule
import com.warrenbrowse.vpn.test.e2e.misc.SomeTrafficToHostRule
import com.warrenbrowse.vpn.test.e2e.misc.SomeTrafficToOtherHostsRule
import com.warrenbrowse.vpn.test.e2e.misc.TrafficGenerator
import com.warrenbrowse.vpn.test.e2e.router.packetCapture.PacketCapture
import com.warrenbrowse.vpn.test.e2e.router.packetCapture.PacketCaptureResult
import org.junit.jupiter.api.BeforeEach
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.extension.RegisterExtension

class LeakTest : EndToEndTest() {

    @RegisterExtension @JvmField val accountTestRule = AccountTestRule()

    @RegisterExtension
    @JvmField
    val forgetAllVpnAppsInSettingsTestRule = ForgetAllVpnAppsInSettingsTestRule()

    val relayProvider = RelayProvider()

    @BeforeEach
    fun setupVPNSettings() {
        app.launchAndLogIn(accountTestRule.validAccountNumber)

        runBlocking {
            app.applySettings(
                localNetworkSharing = true,
                obfuscationMode = ObfuscationMode.Quic,
            )
        }

        on<ConnectPage>()
    }

    @Test
    @HasDependencyOnLocalAPI
    fun testEnsureNoLeaksToSpecificHost() = runTest {
        app.launch()

        on<ConnectPage> {
            waitForDisconnectedLabel()

            clickSelectLocation()
        }

        on<SelectLocationPage> {
            clickLocationExpandButton(relayProvider.getDefaultRelay().country)
            clickLocationExpandButton(relayProvider.getDefaultRelay().city)
            clickLocationCell(relayProvider.getDefaultRelay().relay)
        }

        device.acceptVpnPermissionDialog()

        on<ConnectPage> { waitForConnectedLabel() }

        // Capture generated traffic to a specific host
        val targetIpAddress = InstrumentationRegistry.getArguments().getTrafficGeneratorHost()
        val targetPort = InstrumentationRegistry.getArguments().getTrafficGeneratorPort()
        val captureResult =
            PacketCapture().capturePackets {
                TrafficGenerator(targetIpAddress, targetPort).generateTraffic(10.milliseconds) {
                    // Give it some time for generating traffic
                    delay(3000)
                }
            }

        on<ConnectPage> { clickDisconnect() }

        val capturedStreams = captureResult.streams
        val capturedPcap = captureResult.pcap
        val timestamp = System.currentTimeMillis()
        Attachment.saveAttachment(
            "capture-${javaClass.enclosingMethod}-$timestamp.pcap",
            capturedPcap,
        )

        NetworkTrafficChecker.checkTrafficStreamsAgainstRules(
            capturedStreams,
            NoTrafficToHostRule(targetIpAddress),
        )
    }

    @Test
    @HasDependencyOnLocalAPI
    fun testEnsureLeaksToSpecificHost() = runTest {
        app.launch()

        on<ConnectPage> {
            waitForDisconnectedLabel()

            clickSelectLocation()
        }

        on<SelectLocationPage> {
            clickLocationExpandButton(relayProvider.getDefaultRelay().country)
            clickLocationExpandButton(relayProvider.getDefaultRelay().city)
            clickLocationCell(relayProvider.getDefaultRelay().relay)
        }

        device.acceptVpnPermissionDialog()

        on<ConnectPage> { waitForConnectedLabel() }

        // Capture generated traffic to a specific host
        val targetIpAddress = InstrumentationRegistry.getArguments().getTrafficGeneratorHost()
        val targetPort = InstrumentationRegistry.getArguments().getTrafficGeneratorPort()
        val captureResult: PacketCaptureResult =
            PacketCapture().capturePackets {
                TrafficGenerator(targetIpAddress, targetPort).generateTraffic(10.milliseconds) {
                    delay(3000.milliseconds) // Give it some time for generating traffic in tunnel

                    on<ConnectPage> { clickDisconnect() }

                    delay(2000.milliseconds) // Give it some time to leak traffic outside of tunnel

                    on<ConnectPage> {
                        clickConnect()
                        waitForConnectedLabel()
                    }

                    delay(3000.milliseconds) // Give it some time for generating traffic in tunnel
                }
            }

        on<ConnectPage> { clickDisconnect() }

        val capturedStreams = captureResult.streams
        val capturedPcap = captureResult.pcap
        val timestamp = System.currentTimeMillis()
        Attachment.saveAttachment(
            "capture-${javaClass.enclosingMethod}-$timestamp.pcap",
            capturedPcap,
        )

        NetworkTrafficChecker.checkTrafficStreamsAgainstRules(
            capturedStreams,
            SomeTrafficToHostRule(targetIpAddress),
            SomeTrafficToOtherHostsRule(targetIpAddress),
        )
    }

    @Test
    @HasDependencyOnLocalAPI
    fun testEnsureNoLeaksToSpecificHostWhenSwitchingBetweenVariousVpnSettings() =
        runTest(timeout = 2.minutes) {
            app.launch()
            // Obfuscation and Post-Quantum are by default set to automatic. Explicitly set to off.
            app.applySettings(pq = QuantumResistantState.Off, obfuscationMode = ObfuscationMode.Off)

            on<ConnectPage> { clickSelectLocation() }

            on<SelectLocationPage> {
                clickLocationExpandButton(relayProvider.getDaitaRelay().country)
                clickLocationExpandButton(relayProvider.getDaitaRelay().city)
                clickLocationCell(relayProvider.getDaitaRelay().relay)
            }

            device.acceptVpnPermissionDialog()

            on<ConnectPage> { waitForConnectedLabel() }

            // Capture generated traffic to a specific host
            val targetIpAddress = InstrumentationRegistry.getArguments().getTrafficGeneratorHost()
            val targetPort = InstrumentationRegistry.getArguments().getTrafficGeneratorPort()
            val captureResult: PacketCaptureResult =
                PacketCapture().capturePackets {
                    TrafficGenerator(targetIpAddress, targetPort).generateTraffic(10.milliseconds) {
                        delay(
                            1000.milliseconds
                        ) // Give it some time for generating traffic in tunnel before changing
                        // settings

                        on<ConnectPage> { enableDAITAStory() }
                        app.applySettings(obfuscationMode = ObfuscationMode.Shadowsocks)
                        on<ConnectPage> { waitForConnectedLabel() }

                        delay(
                            1000.milliseconds
                        ) // Give it some time for generating traffic in tunnel after enabling
                        // settings
                    }
                }

            val capturedStreams = captureResult.streams
            val capturedPcap = captureResult.pcap
            val timestamp = System.currentTimeMillis()
            Attachment.saveAttachment(
                "capture-${javaClass.enclosingMethod}-$timestamp.pcap",
                capturedPcap,
            )

            NetworkTrafficChecker.checkTrafficStreamsAgainstRules(
                capturedStreams,
                NoTrafficToHostRule(targetIpAddress),
            )
        }
}
