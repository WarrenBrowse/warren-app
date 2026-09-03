package com.warrenbrowse.vpn.app.forum

import android.net.ConnectivityManager
import com.warrenbrowse.vpn.lib.model.forum.ForumIdentity
import com.warrenbrowse.vpn.lib.model.wallet.Mnemonic
import com.warrenbrowse.vpn.lib.model.wallet.SensitiveOpAuthorizer
import com.warrenbrowse.vpn.lib.model.wallet.WalletAddress
import com.warrenbrowse.vpn.lib.model.wallet.WalletState
import com.warrenbrowse.vpn.lib.repository.ForumIdentityRepository
import com.warrenbrowse.vpn.lib.repository.WalletRepository
import com.warrenbrowse.vpn.lib.repository.WarrenConnectedInfo
import com.warrenbrowse.vpn.lib.repository.WarrenJniBridge
import com.warrenbrowse.vpn.lib.repository.WarrenVersionVerdict
import com.warrenbrowse.vpn.lib.repository.WarrenTunnelStateProvider
import java.time.Instant
import java.util.Locale
import java.util.TimeZone
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/** A real Warren SS58 vector (49 chars, prefix 13295), the wallet the fakes hold. */
internal const val TEST_ADDRESS = "wb7kgy8FF4rx4tamkksPfoymeeeZVXLrnSjbBxCun3XhP9DnB"

internal const val TEST_PHRASE =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"

/** A wallet on disk whose silent reads are counted, so a test can prove none happened. */
internal class FakeWalletRepository(
    initial: WalletState = WalletState.Locked(WalletAddress(TEST_ADDRESS))
) : WalletRepository {
    val stateFlow = MutableStateFlow(initial)
    override val state: StateFlow<WalletState> = stateFlow.asStateFlow()
    var mnemonicReads = 0

    override suspend fun createWallet(authorizer: SensitiveOpAuthorizer?): Mnemonic =
        error("unused")

    override suspend fun importWallet(
        mnemonic: Mnemonic,
        authorizer: SensitiveOpAuthorizer?,
    ): WalletAddress = error("unused")

    override suspend fun unlock(authorizer: SensitiveOpAuthorizer, reason: String): Mnemonic =
        error("unused")

    override suspend fun readMnemonic(): Mnemonic {
        mnemonicReads++
        return Mnemonic(TEST_PHRASE)
    }

    override suspend fun erase() {
        stateFlow.value = WalletState.Absent
    }
}

internal class FakeForumIdentityRepository : ForumIdentityRepository {
    private val _identity = MutableStateFlow<ForumIdentity?>(null)
    override val identity: StateFlow<ForumIdentity?> = _identity.asStateFlow()

    override fun save(identity: ForumIdentity) {
        _identity.value = identity
    }

    override fun clear() {
        _identity.value = null
    }
}

internal class FakeTunnelStateProvider(
    initial: WarrenConnectedInfo = WarrenConnectedInfo.Disconnected,
    stateText: String = "",
) : WarrenTunnelStateProvider {
    val info = MutableStateFlow(initial)
    override val state: StateFlow<String> = MutableStateFlow(stateText).asStateFlow()
    override val connectedInfo: StateFlow<WarrenConnectedInfo> = info.asStateFlow()
}

/** A journal kept in memory, so a test reads back exactly what was recorded. */
internal class RecordingJournal : ForumJournal {
    val entries = mutableListOf<Pair<ForumEvent, List<JournalField>>>()

    override fun record(event: ForumEvent, vararg fields: JournalField) {
        entries += event to fields.toList()
    }

    override suspend fun lastClassOf(event: ForumEvent): String? =
        entries
            .filter { it.first == event }
            .mapNotNull { (_, fields) -> fields.filterIsInstance<JournalField.Class>().firstOrNull() }
            .lastOrNull()
            ?.value

    fun fieldsOf(event: ForumEvent): List<List<JournalField>> =
        entries.filter { it.first == event }.map { it.second }
}

/**
 * The JNI seam with every network export answering from the test, and counted:
 * the point of most tests here is that a call did or did not cross into Rust.
 */
internal class FakeJniBridge(
    private val loginAnswer: () -> String = { """{"ok":true}""" },
    private val reportAnswer: () -> String = {
        """{"ok":true,"topic_id":1,"topic_url":"","logs":"none"}"""
    },
    private val collectAnswer: () -> String = { """{"ok":true,"bytes":7}""" },
) : WarrenJniBridge {
    var loginCalls = 0
    var cancelCalls = 0
    var reportCalls = 0
    val collectedMetadata = mutableListOf<String>()
    val collectedForSend = mutableListOf<Boolean>()

    override fun generateMnemonic(): String = error("unused")

    override fun mnemonicPubkeySs58(mnemonic: String): String = error("unused")

    override fun fetchVersionInfo(currentVersion: String): WarrenVersionVerdict = error("unused")

    override fun fetchNetworkInfo(): String = error("unused")

    override fun forumLogin(mnemonic: String, sid: String, host: String): String {
        loginCalls++
        return loginAnswer()
    }

    override fun forumLoginCancel(sid: String, host: String) {
        cancelCalls++
    }

    override fun forumReport(mnemonic: String, reportJson: String, logGz: ByteArray?): String {
        reportCalls++
        return reportAnswer()
    }

    override fun collectProblemReport(
        metadataJson: String,
        redactJson: String,
        appLogDir: String,
        outputPath: String,
        forSend: Boolean,
    ): String {
        collectedMetadata += metadataJson
        collectedForSend += forSend
        return collectAnswer()
    }
}

/**
 * A stock Pixel as the platform reads describe it, every value settable and
 * any read made to throw by name, so the fact table is exercised on the
 * device shapes the header exists for.
 */
internal class FakePlatformReads : ForumPlatformReads {
    override val packageName = "com.warrenbrowse.vpn.beta"
    var installer: String? = "com.android.vending"
    var buildFacts =
        BuildFacts(
            fingerprint = "google/panther/panther:14/UP1A.231005.007/1:user/release-keys",
            display = "UP1A.231005.007",
            securityPatch = "2024-01-05",
            tags = "release-keys",
            type = "user",
        )
    val properties = mutableMapOf<String, String>()
    val packages =
        mutableMapOf<String, String?>(
            "com.google.android.gms" to "24.02.13",
            "com.android.vending" to "39.2.19",
            "org.mozilla.firefox" to "154.0.1",
        )
    var webView: InstalledPackage? = InstalledPackage("com.google.android.webview", "120.0.6099.230")
    var now: Instant = Instant.parse("2026-09-02T21:38:46Z")
    var zone: TimeZone = TimeZone.getTimeZone("Europe/Paris")
    var locale: Locale = Locale.FRANCE
    val globalInts = mutableMapOf("auto_time" to 1, "auto_time_zone" to 1, "airplane_mode_on" to 0)
    val globalStrings = mutableMapOf("private_dns_mode" to "opportunistic")
    val secureInts = mutableMapOf("always_on_vpn_lockdown" to 1)
    val secureStrings = mutableMapOf("always_on_vpn_app" to packageName)
    var elapsedMs = 90_000L
    var processStartMs = 30_000L
    var handlers = listOf(packageName)
    var resolved: String? = packageName
    var browser: String? = "org.mozilla.firefox"
    var powerFacts = PowerFacts(ignoringBatteryOptimisations = false, powerSave = false, deviceIdle = false)
    var backgroundFacts = BackgroundFacts(restricted = false, lowRam = false)
    var bucket = 10
    var restrictBackground = ConnectivityManager.RESTRICT_BACKGROUND_STATUS_DISABLED
    var vpnPrepared = true
    var network: NetworkFacts? =
        NetworkFacts(
            transports = setOf(NetworkTransport.WIFI, NetworkTransport.VPN),
            flags = setOf(NetworkFlag.VALIDATED, NetworkFlag.INTERNET),
            mtu = 1280,
            linkAddresses = listOf("10.66.0.2", "fd00:66::2"),
            dnsServers = listOf("10.66.0.1"),
            ssid = null,
            privateDnsActive = false,
            privateDnsServerName = null,
        )

    /** Reads that throw, by method name. */
    val failing = mutableMapOf<String, Exception>()

    /** Every link the table asked the system to resolve. */
    val probedLinks = mutableListOf<String>()

    private fun <T> read(name: String, value: () -> T): T {
        failing[name]?.let { throw it }
        return value()
    }

    override fun installerPackage(): String? = read("installerPackage") { installer }

    override fun build(): BuildFacts = read("build") { buildFacts }

    override fun systemProperty(name: String): String? = read("systemProperty") { properties[name] }

    override fun installedPackage(packageName: String): InstalledPackage? =
        read("installedPackage") {
            if (packageName in packages) InstalledPackage(packageName, packages[packageName]) else null
        }

    override fun webViewPackage(): InstalledPackage? = read("webViewPackage") { webView }

    override fun now(): Instant = read("now") { now }

    override fun timeZone(): TimeZone = read("timeZone") { zone }

    override fun locale(): Locale = read("locale") { locale }

    override fun globalSettingInt(name: String): Int = read("globalSettingInt") { globalInts[name] ?: -1 }

    override fun globalSettingString(name: String): String? =
        read("globalSettingString") { globalStrings[name] }

    override fun secureSettingInt(name: String): Int = read("secureSettingInt") { secureInts[name] ?: -1 }

    override fun secureSettingString(name: String): String? =
        read("secureSettingString") { secureStrings[name] }

    override fun elapsedRealtimeMs(): Long = read("elapsedRealtimeMs") { elapsedMs }

    override fun processStartElapsedMs(): Long = read("processStartElapsedMs") { processStartMs }

    override fun deepLinkHandlers(link: String): List<String> =
        read("deepLinkHandlers") {
            probedLinks += link
            handlers
        }

    override fun deepLinkResolvedPackage(link: String): String? =
        read("deepLinkResolvedPackage") {
            probedLinks += link
            resolved
        }

    override fun defaultBrowserPackage(url: String): String? = read("defaultBrowserPackage") { browser }

    override fun power(): PowerFacts = read("power") { powerFacts }

    override fun background(): BackgroundFacts = read("background") { backgroundFacts }

    override fun standbyBucket(): Int = read("standbyBucket") { bucket }

    override fun restrictBackgroundStatus(): Int = read("restrictBackgroundStatus") { restrictBackground }

    override fun vpnServicePrepared(): Boolean = read("vpnServicePrepared") { vpnPrepared }

    override fun activeNetwork(): NetworkFacts? = read("activeNetwork") { network }
}
