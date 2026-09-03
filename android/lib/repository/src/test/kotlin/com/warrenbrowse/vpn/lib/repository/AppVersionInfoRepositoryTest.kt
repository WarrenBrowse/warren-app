package com.warrenbrowse.vpn.lib.repository

import com.warrenbrowse.vpn.lib.model.BuildVersion
import kotlin.time.Duration.Companion.days
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.test.runTest
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

/**
 * The manifest is fetched and verified in Rust; what this side owns is how
 * many times per check it asks, and what the two answers default to when the
 * ask fails. Both used to be separate fetches of the same signed file.
 */
class AppVersionInfoRepositoryTest {

    private class CountingBridge(private val answer: () -> WarrenVersionVerdict) :
        WarrenJniBridge {
        var calls = 0

        override fun fetchVersionInfo(currentVersion: String): WarrenVersionVerdict {
            calls++
            return answer()
        }

        override fun generateMnemonic(): String = error("unused")

        override fun mnemonicPubkeySs58(mnemonic: String): String = error("unused")

        override fun fetchNetworkInfo(): String = error("unused")

        override fun forumLogin(mnemonic: String, sid: String, host: String): String =
            error("unused")

        override fun forumLoginCancel(sid: String, host: String) = error("unused")

        override fun forumReport(mnemonic: String, reportJson: String, logGz: ByteArray?): String =
            error("unused")

        override fun forumDigestFetch(): String = error("unused")

        override fun noticesFetch(currentVersion: String): String = error("unused")

        override fun announcementsFetch(currentVersion: String): String = error("unused")

        override fun campaignVoucher(mnemonic: String, campaignId: String): String =
            error("unused")

        override fun forumNotifications(mnemonic: String): String = error("unused")

        override fun forumNotificationsSeen(mnemonic: String): String = error("unused")

        override fun reportPubkeyMismatch(
            mnemonic: String,
            exitIdHex: String,
            oldPubkeyHex: String,
            newPubkeyHex: String,
            countryCode: String,
            city: String,
        ): String = error("unused")

        override fun collectProblemReport(
            metadataJson: String,
            redactJson: String,
            appLogDir: String,
            outputPath: String,
            forSend: Boolean,
        ): String = error("unused")
    }

    private val build = BuildVersion("1.0.0", 1)

    // Unconfined so the construction-time refresh completes before the
    // constructor returns; the periodic timer then parks for a year.
    private fun repository(bridge: CountingBridge) =
        AppVersionInfoRepository(
            buildVersion = build,
            jniBridge = bridge,
            ioDispatcher = Dispatchers.Unconfined,
            refreshInterval = 365.days,
        )

    @Test
    fun `ensure one manifest read answers both the gate and the prompt`() = runTest {
        val bridge =
            CountingBridge { WarrenVersionVerdict(isSupported = false, latestAvailable = "1.2.0") }

        val repository = repository(bridge)

        assertEquals(1, bridge.calls, "the construction-time check reads the manifest once")
        val info = repository.versionInfo.value
        assertFalse(info.isSupported)
        assertEquals("1.2.0", info.availableUpgrade)
        assertEquals("1.0.0", info.currentVersion)
    }

    @Test
    fun `ensure an explicit refresh reads the manifest exactly once more`() = runTest {
        val bridge =
            CountingBridge { WarrenVersionVerdict(isSupported = true, latestAvailable = null) }
        val repository = repository(bridge)

        repository.refresh()

        assertEquals(2, bridge.calls)
    }

    @Test
    fun `ensure a failed read fails open on support and closed on the prompt`() = runTest {
        val bridge = CountingBridge { error("bridge down") }

        val repository = repository(bridge)

        val info = repository.versionInfo.value
        assertTrue(info.isSupported, "a flaky network must never lock the user out")
        assertNull(info.availableUpgrade, "a prompt for an update that may not exist")
    }
}
