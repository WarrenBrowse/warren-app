package com.warrenbrowse.vpn.app.forum

import android.app.ActivityManager
import android.app.usage.UsageStatsManager
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.ConnectivityManager
import android.net.NetworkCapabilities
import android.net.Uri
import android.os.Build
import android.os.PowerManager
import android.os.SystemClock
import android.provider.Settings
import android.webkit.WebView
import com.warrenbrowse.vpn.BuildConfig
import java.time.Instant
import java.util.Locale
import java.util.TimeZone

/**
 * The platform facts a problem report needs to explain a forum sign-in that
 * never reached the broker, read with the Android APIs only Kotlin has. Each
 * fact is read on its own and a failure records `unreadable`, so one ROM
 * without a setting never costs the rest of the header.
 *
 * Every value here is safe by construction (the header block is not
 * redacted): no address, no SSID, no sid, no handle. Only classes,
 * settings, package names and versions.
 */
class ForumDiagnostics(private val context: Context) {

    fun collect(tunnelState: String, walletState: String, lastLoginClass: String?): Map<String, String> {
        val facts = linkedMapOf<String, String>()
        fun put(key: String, read: () -> String?) {
            facts[key] =
                try {
                    read() ?: "none"
                } catch (e: Exception) {
                    "unreadable:${e.javaClass.simpleName}"
                }
        }

        // Build and identity of the app.
        put("report-schema") { "android/1" }
        put("warren-product-env") { BuildConfig.FLAVOR }
        put("android-application-id") { BuildConfig.APPLICATION_ID }
        put("android-build-type") { "${BuildConfig.BUILD_TYPE} versionCode=${BuildConfig.VERSION_CODE}" }
        put("deep-link-scheme") { BuildConfig.DEEP_LINK_SCHEME }
        put("installer-package") {
            val pm = context.packageManager
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
                pm.getInstallSourceInfo(context.packageName).installingPackageName ?: "unknown"
            } else {
                @Suppress("DEPRECATION")
                pm.getInstallerPackageName(context.packageName) ?: "unknown"
            }
        }

        // Device, ROM and Google services.
        put("android-fingerprint") {
            "${Build.FINGERPRINT} display=${Build.DISPLAY} patch=${Build.VERSION.SECURITY_PATCH}"
        }
        put("android-rom") { romVerdict() }
        put("gms") { gmsVerdict() }
        put("webview-package") {
            val pkg = WebView.getCurrentWebViewPackage()
            if (pkg == null) "none" else "${pkg.packageName} ${pkg.versionName}"
        }

        // Clock.
        put("time-now-utc") { Instant.now().toString() }
        put("time-zone") {
            val tz = TimeZone.getDefault()
            "${tz.id} offset=${tz.rawOffset / 60_000}min"
        }
        put("time-auto") {
            val auto = Settings.Global.getInt(context.contentResolver, Settings.Global.AUTO_TIME, -1)
            val zone =
                Settings.Global.getInt(context.contentResolver, Settings.Global.AUTO_TIME_ZONE, -1)
            "auto_time=${auto.settingWord()} auto_time_zone=${zone.settingWord()}"
        }
        put("uptime") {
            "elapsed=${SystemClock.elapsedRealtime() / 1000}s" +
                " process_age=${(SystemClock.elapsedRealtime() - android.os.Process.getStartElapsedRealtime()) / 1000}s"
        }
        put("locale") { Locale.getDefault().toLanguageTag() }

        // Deep-link routing.
        put("deep-link-handlers") { deepLinkHandlers() }
        put("deep-link-resolved") { deepLinkResolved() }
        put("default-browser") { defaultBrowser() }
        put("last-forum-login") { lastLoginClass ?: "none" }

        // Background and power restrictions.
        put("battery-optimisation") {
            val pm = context.getSystemService(Context.POWER_SERVICE) as PowerManager
            "ignoring=${pm.isIgnoringBatteryOptimizations(context.packageName)}" +
                " power_save=${pm.isPowerSaveMode} idle=${pm.isDeviceIdleMode}"
        }
        put("background-restricted") {
            val am = context.getSystemService(Context.ACTIVITY_SERVICE) as ActivityManager
            "${am.isBackgroundRestricted} low_ram=${am.isLowRamDevice}"
        }
        put("standby-bucket") {
            val usm = context.getSystemService(Context.USAGE_STATS_SERVICE) as UsageStatsManager
            usm.appStandbyBucket.toString()
        }
        put("data-saver") {
            val cm = context.getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager
            when (cm.restrictBackgroundStatus) {
                ConnectivityManager.RESTRICT_BACKGROUND_STATUS_DISABLED -> "off"
                ConnectivityManager.RESTRICT_BACKGROUND_STATUS_WHITELISTED -> "on-whitelisted"
                ConnectivityManager.RESTRICT_BACKGROUND_STATUS_ENABLED -> "on"
                else -> "unknown"
            }
        }

        // VPN and tunnel.
        put("tunnel-state") { tunnelState }
        put("vpn-service-prepared") {
            (android.net.VpnService.prepare(context) == null).toString()
        }
        put("always-on") {
            val app = Settings.Secure.getString(context.contentResolver, "always_on_vpn_app")
            val lockdown =
                Settings.Secure.getInt(context.contentResolver, "always_on_vpn_lockdown", -1)
            val ours = app == context.packageName
            "app=${if (app == null) "none" else if (ours) "this" else "other"} lockdown=${lockdown.settingWord()}"
        }
        put("other-vpn") { otherVpnPresent() }

        // Network.
        put("network") { activeNetwork() }
        put("private-dns") { privateDns() }
        put("airplane-mode") {
            Settings.Global.getInt(context.contentResolver, Settings.Global.AIRPLANE_MODE_ON, -1)
                .settingWord()
        }

        // Wallet.
        put("wallet") { walletState }
        return facts
    }

    private fun Int.settingWord(): String =
        when (this) {
            1 -> "1"
            0 -> "0"
            else -> "unreadable"
        }

    private fun romVerdict(): String {
        val lineage = prop("ro.lineage.version") ?: prop("ro.lineage.build.version")
        val eos =
            listOf("foundation.e.apps", "foundation.e.browser", "foundation.e.blisslauncher").any {
                installed(it)
            }
        val verdict =
            when {
                eos -> "e-os"
                lineage != null -> "lineage"
                installed("app.grapheneos.apps") -> "graphene"
                Build.TAGS?.contains("release-keys") == true -> "stock"
                else -> "other"
            }
        return "$verdict tags=${Build.TAGS} type=${Build.TYPE} lineage=${lineage ?: "none"}"
    }

    private fun gmsVerdict(): String {
        val gms = installed("com.google.android.gms")
        val microg = installed("org.microg.gms") || installed("com.mgoogle.android.gms")
        val vending = installed("com.android.vending")
        val version =
            try {
                context.packageManager.getPackageInfo("com.google.android.gms", 0).versionName
            } catch (e: PackageManager.NameNotFoundException) {
                null
            }
        val kind =
            when {
                microg -> "microg"
                gms && (version?.startsWith("0.") == true) -> "microg"
                gms -> "google"
                else -> "none"
            }
        return "$kind vending=$vending version=${version ?: "none"}"
    }

    private fun sampleLink(): Intent =
        Intent(
            Intent.ACTION_VIEW,
            Uri.parse(
                "${BuildConfig.DEEP_LINK_SCHEME}://forum-login?sid=00000000000000000000000000000000&host=connect.warrenbrowse.com"
            ),
        ).addCategory(Intent.CATEGORY_BROWSABLE)

    private fun deepLinkHandlers(): String {
        val pm = context.packageManager
        val handlers =
            pm.queryIntentActivities(sampleLink(), PackageManager.MATCH_DEFAULT_ONLY)
                .map { it.activityInfo.packageName }
        val ours = context.packageName in handlers
        return "ours=$ours count=${handlers.size} ${handlers.joinToString(",")}"
    }

    private fun deepLinkResolved(): String {
        val resolved = sampleLink().resolveActivity(context.packageManager)
        return when (resolved?.packageName) {
            null -> "none"
            context.packageName -> "this"
            "android" -> "chooser"
            else -> "other"
        }
    }

    private fun defaultBrowser(): String {
        val probe = Intent(Intent.ACTION_VIEW, Uri.parse("https://forum.warrenbrowse.com/"))
        val resolved =
            context.packageManager.resolveActivity(probe, PackageManager.MATCH_DEFAULT_ONLY)
        val pkg = resolved?.activityInfo?.packageName ?: return "none"
        val version =
            try {
                context.packageManager.getPackageInfo(pkg, 0).versionName
            } catch (e: PackageManager.NameNotFoundException) {
                null
            }
        return "$pkg ${version ?: ""}".trim()
    }

    private fun otherVpnPresent(): String {
        val cm = context.getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager
        val active = cm.activeNetwork?.let(cm::getNetworkCapabilities)
        val activeIsVpn = active?.hasTransport(NetworkCapabilities.TRANSPORT_VPN) == true
        return "active_is_vpn=$activeIsVpn"
    }

    private fun activeNetwork(): String {
        val cm = context.getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager
        val network = cm.activeNetwork ?: return "none"
        val caps = cm.getNetworkCapabilities(network) ?: return "no-capabilities"
        val transports =
            listOf(
                    NetworkCapabilities.TRANSPORT_WIFI to "wifi",
                    NetworkCapabilities.TRANSPORT_CELLULAR to "cellular",
                    NetworkCapabilities.TRANSPORT_ETHERNET to "ethernet",
                    NetworkCapabilities.TRANSPORT_VPN to "vpn",
                )
                .filter { caps.hasTransport(it.first) }
                .joinToString("+") { it.second }
        val flags =
            listOf(
                    NetworkCapabilities.NET_CAPABILITY_VALIDATED to "validated",
                    NetworkCapabilities.NET_CAPABILITY_CAPTIVE_PORTAL to "captive",
                    NetworkCapabilities.NET_CAPABILITY_NOT_METERED to "unmetered",
                    NetworkCapabilities.NET_CAPABILITY_INTERNET to "internet",
                )
                .filter { caps.hasCapability(it.first) }
                .joinToString(",") { it.second }
        val props = cm.getLinkProperties(network)
        val v4 = props?.linkAddresses?.count { it.address is java.net.Inet4Address } ?: 0
        val v6 = props?.linkAddresses?.count { it.address is java.net.Inet6Address } ?: 0
        return "$transports [$flags] mtu=${props?.mtu ?: 0} v4=$v4 v6=$v6 dns=${props?.dnsServers?.size ?: 0}"
    }

    private fun privateDns(): String {
        val cm = context.getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager
        val props = cm.activeNetwork?.let(cm::getLinkProperties) ?: return "no-network"
        val mode = Settings.Global.getString(context.contentResolver, "private_dns_mode") ?: "unset"
        return "active=${props.isPrivateDnsActive} mode=$mode named=${props.privateDnsServerName != null}"
    }

    private fun installed(pkg: String): Boolean =
        try {
            context.packageManager.getPackageInfo(pkg, 0)
            true
        } catch (e: PackageManager.NameNotFoundException) {
            false
        }

    private fun prop(name: String): String? =
        try {
            val cls = Class.forName("android.os.SystemProperties")
            val get = cls.getMethod("get", String::class.java)
            (get.invoke(null, name) as? String)?.takeIf { it.isNotBlank() }
        } catch (e: Exception) {
            null
        }
}
