package com.warrenbrowse.vpn.app.forum

import android.app.ActivityManager
import android.app.usage.UsageStatsManager
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.ConnectivityManager
import android.net.NetworkCapabilities
import android.net.Uri
import android.net.VpnService
import android.os.Build
import android.os.PowerManager
import android.os.Process
import android.os.SystemClock
import android.provider.Settings
import android.webkit.WebView
import java.net.Inet4Address
import java.net.Inet6Address
import java.time.Instant
import java.util.Locale
import java.util.TimeZone

/** A package as the package manager reports it. */
data class InstalledPackage(val packageName: String, val versionName: String?)

/** The build identity of the ROM. */
data class BuildFacts(
    val fingerprint: String,
    val display: String,
    val securityPatch: String?,
    val tags: String?,
    val type: String,
)

data class PowerFacts(
    val ignoringBatteryOptimisations: Boolean,
    val powerSave: Boolean,
    val deviceIdle: Boolean,
)

data class BackgroundFacts(val restricted: Boolean, val lowRam: Boolean)

enum class NetworkTransport(val word: String) {
    WIFI("wifi"),
    CELLULAR("cellular"),
    ETHERNET("ethernet"),
    VPN("vpn"),
}

enum class NetworkFlag(val word: String) {
    VALIDATED("validated"),
    CAPTIVE("captive"),
    UNMETERED("unmetered"),
    INTERNET("internet"),
}

/**
 * The active network as the platform describes it. The link addresses, the
 * DNS servers and the private DNS host name are carried raw so the header can
 * count or test them; the fact table never prints them. The SSID has the same
 * standing and no reader: the header has no use for a network name, and the
 * device never reads one.
 */
data class NetworkFacts(
    val transports: Set<NetworkTransport>,
    val flags: Set<NetworkFlag>,
    val mtu: Int,
    val linkAddresses: List<String>,
    val dnsServers: List<String>,
    val ssid: String?,
    val privateDnsActive: Boolean,
    val privateDnsServerName: String?,
)

/**
 * The raw platform reads the report header is built from, one per fact so a
 * ROM that refuses one never costs the rest. [AndroidForumPlatformReads] is
 * the device; a test supplies the values.
 */
interface ForumPlatformReads {
    val packageName: String

    fun installerPackage(): String?

    fun build(): BuildFacts

    fun systemProperty(name: String): String?

    /** Null when the package is not installed. */
    fun installedPackage(packageName: String): InstalledPackage?

    /** Null when the device has no WebView provider at all. */
    fun webViewPackage(): InstalledPackage?

    fun now(): Instant

    fun timeZone(): TimeZone

    fun locale(): Locale

    /** -1 when the setting is unset. */
    fun globalSettingInt(name: String): Int

    fun globalSettingString(name: String): String?

    fun secureSettingInt(name: String): Int

    fun secureSettingString(name: String): String?

    fun elapsedRealtimeMs(): Long

    fun processStartElapsedMs(): Long

    /** The packages that offer to open [link], as the default-only resolution lists them. */
    fun deepLinkHandlers(link: String): List<String>

    /** The package the system would hand [link] to, or null when nothing takes it. */
    fun deepLinkResolvedPackage(link: String): String?

    fun defaultBrowserPackage(url: String): String?

    fun power(): PowerFacts

    fun background(): BackgroundFacts

    fun standbyBucket(): Int

    /** A `ConnectivityManager.RESTRICT_BACKGROUND_STATUS_*` value. */
    fun restrictBackgroundStatus(): Int

    fun vpnServicePrepared(): Boolean

    fun activeNetwork(): NetworkFacts?
}

/** The device: each read is the one Android call it names, nothing derived. */
class AndroidForumPlatformReads(private val context: Context) : ForumPlatformReads {
    private val pm: PackageManager
        get() = context.packageManager

    private val cm: ConnectivityManager
        get() = context.getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager

    override val packageName: String
        get() = context.packageName

    override fun installerPackage(): String? =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            pm.getInstallSourceInfo(context.packageName).installingPackageName
        } else {
            @Suppress("DEPRECATION") pm.getInstallerPackageName(context.packageName)
        }

    override fun build(): BuildFacts =
        BuildFacts(
            fingerprint = Build.FINGERPRINT,
            display = Build.DISPLAY,
            securityPatch = Build.VERSION.SECURITY_PATCH,
            tags = Build.TAGS,
            type = Build.TYPE,
        )

    override fun systemProperty(name: String): String? =
        try {
            val cls = Class.forName("android.os.SystemProperties")
            val get = cls.getMethod("get", String::class.java)
            (get.invoke(null, name) as? String)?.takeIf { it.isNotBlank() }
        } catch (e: ReflectiveOperationException) {
            null
        }

    override fun installedPackage(packageName: String): InstalledPackage? =
        try {
            InstalledPackage(packageName, pm.getPackageInfo(packageName, 0).versionName)
        } catch (e: PackageManager.NameNotFoundException) {
            null
        }

    override fun webViewPackage(): InstalledPackage? =
        WebView.getCurrentWebViewPackage()?.let { InstalledPackage(it.packageName, it.versionName) }

    override fun now(): Instant = Instant.now()

    override fun timeZone(): TimeZone = TimeZone.getDefault()

    override fun locale(): Locale = Locale.getDefault()

    override fun globalSettingInt(name: String): Int =
        Settings.Global.getInt(context.contentResolver, name, -1)

    override fun globalSettingString(name: String): String? =
        Settings.Global.getString(context.contentResolver, name)

    override fun secureSettingInt(name: String): Int =
        Settings.Secure.getInt(context.contentResolver, name, -1)

    override fun secureSettingString(name: String): String? =
        Settings.Secure.getString(context.contentResolver, name)

    override fun elapsedRealtimeMs(): Long = SystemClock.elapsedRealtime()

    override fun processStartElapsedMs(): Long = Process.getStartElapsedRealtime()

    override fun deepLinkHandlers(link: String): List<String> =
        pm.queryIntentActivities(viewIntent(link), PackageManager.MATCH_DEFAULT_ONLY).map {
            it.activityInfo.packageName
        }

    override fun deepLinkResolvedPackage(link: String): String? =
        viewIntent(link).resolveActivity(pm)?.packageName

    override fun defaultBrowserPackage(url: String): String? =
        pm.resolveActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url)), PackageManager.MATCH_DEFAULT_ONLY)
            ?.activityInfo
            ?.packageName

    override fun power(): PowerFacts {
        val power = context.getSystemService(Context.POWER_SERVICE) as PowerManager
        return PowerFacts(
            ignoringBatteryOptimisations = power.isIgnoringBatteryOptimizations(context.packageName),
            powerSave = power.isPowerSaveMode,
            deviceIdle = power.isDeviceIdleMode,
        )
    }

    override fun background(): BackgroundFacts {
        val am = context.getSystemService(Context.ACTIVITY_SERVICE) as ActivityManager
        return BackgroundFacts(restricted = am.isBackgroundRestricted, lowRam = am.isLowRamDevice)
    }

    override fun standbyBucket(): Int =
        (context.getSystemService(Context.USAGE_STATS_SERVICE) as UsageStatsManager).appStandbyBucket

    override fun restrictBackgroundStatus(): Int = cm.restrictBackgroundStatus

    override fun vpnServicePrepared(): Boolean = VpnService.prepare(context) == null

    override fun activeNetwork(): NetworkFacts? {
        val network = cm.activeNetwork
        val caps = network?.let(cm::getNetworkCapabilities) ?: return null
        val props = cm.getLinkProperties(network)
        val addresses = props?.linkAddresses.orEmpty().map { it.address }
        return NetworkFacts(
            transports = TRANSPORTS.filter { caps.hasTransport(it.first) }.mapTo(linkedSetOf()) { it.second },
            flags = FLAGS.filter { caps.hasCapability(it.first) }.mapTo(linkedSetOf()) { it.second },
            mtu = props?.mtu ?: 0,
            linkAddresses =
                addresses.filter { it is Inet4Address || it is Inet6Address }.map { it.hostAddress ?: "" },
            dnsServers = props?.dnsServers.orEmpty().map { it.hostAddress ?: "" },
            ssid = null,
            privateDnsActive = props?.isPrivateDnsActive == true,
            privateDnsServerName = props?.privateDnsServerName,
        )
    }

    private fun viewIntent(link: String): Intent =
        Intent(Intent.ACTION_VIEW, Uri.parse(link)).addCategory(Intent.CATEGORY_BROWSABLE)

    private companion object {
        // In the order the header words them, wifi first.
        val TRANSPORTS =
            listOf(
                NetworkCapabilities.TRANSPORT_WIFI to NetworkTransport.WIFI,
                NetworkCapabilities.TRANSPORT_CELLULAR to NetworkTransport.CELLULAR,
                NetworkCapabilities.TRANSPORT_ETHERNET to NetworkTransport.ETHERNET,
                NetworkCapabilities.TRANSPORT_VPN to NetworkTransport.VPN,
            )
        val FLAGS =
            listOf(
                NetworkCapabilities.NET_CAPABILITY_VALIDATED to NetworkFlag.VALIDATED,
                NetworkCapabilities.NET_CAPABILITY_CAPTIVE_PORTAL to NetworkFlag.CAPTIVE,
                NetworkCapabilities.NET_CAPABILITY_NOT_METERED to NetworkFlag.UNMETERED,
                NetworkCapabilities.NET_CAPABILITY_INTERNET to NetworkFlag.INTERNET,
            )
    }
}
