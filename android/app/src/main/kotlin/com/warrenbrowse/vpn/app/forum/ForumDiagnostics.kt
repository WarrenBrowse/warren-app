package com.warrenbrowse.vpn.app.forum

import android.net.ConnectivityManager
import android.provider.Settings
import com.warrenbrowse.vpn.BuildConfig

/**
 * The platform facts a problem report needs to explain a forum sign-in that
 * never reached the broker, read through [ForumPlatformReads]. Each fact is
 * read on its own and a failure records `unreadable`, so one ROM without a
 * setting never costs the rest of the header.
 *
 * Every value here is safe by construction (the header block is not
 * redacted): no address, no SSID, no sid, no handle. Only classes, settings,
 * counts, package names and versions.
 */
class ForumDiagnostics(private val reads: ForumPlatformReads) : ForumFacts {

    override fun collect(
        tunnelState: String,
        walletState: String,
        lastLoginClass: String?,
    ): Map<String, String> {
        val facts = linkedMapOf<String, String>()
        fun put(key: String, read: () -> String?) {
            facts[key] =
                try {
                    read() ?: NONE
                } catch (e: Exception) {
                    "unreadable:${e.javaClass.simpleName}"
                }
        }

        // Build and identity of the app.
        put("report-schema") { "android/1" }
        put("warren-product-env") { BuildConfig.FLAVOR }
        put("android-application-id") { BuildConfig.APPLICATION_ID }
        put("android-build-type") {
            "${BuildConfig.BUILD_TYPE} versionCode=${BuildConfig.VERSION_CODE}"
        }
        put("deep-link-scheme") { BuildConfig.DEEP_LINK_SCHEME }
        put("installer-package") { reads.installerPackage() ?: "unknown" }

        // Device, ROM and Google services.
        put("android-fingerprint") {
            val build = reads.build()
            "${build.fingerprint} display=${build.display} patch=${build.securityPatch}"
        }
        put("android-rom") { romVerdict() }
        put("gms") { gmsVerdict() }
        put("webview-package") {
            reads.webViewPackage()?.let { "${it.packageName} ${it.versionName}" }
        }

        // Clock.
        put("time-now-utc") { reads.now().toString() }
        put("time-zone") {
            val tz = reads.timeZone()
            "${tz.id} offset=${tz.rawOffset / MS_PER_MINUTE}min"
        }
        put("time-auto") {
            val auto = reads.globalSettingInt(Settings.Global.AUTO_TIME)
            val zone = reads.globalSettingInt(Settings.Global.AUTO_TIME_ZONE)
            "auto_time=${auto.settingWord()} auto_time_zone=${zone.settingWord()}"
        }
        put("uptime") {
            val elapsed = reads.elapsedRealtimeMs()
            "elapsed=${elapsed / MS_PER_SECOND}s" +
                " process_age=${(elapsed - reads.processStartElapsedMs()) / MS_PER_SECOND}s"
        }
        put("locale") { reads.locale().toLanguageTag() }

        // Deep-link routing.
        put("deep-link-handlers") { deepLinkHandlers() }
        put("deep-link-resolved") { deepLinkResolved() }
        put("default-browser") { defaultBrowser() }
        put("last-forum-login") { lastLoginClass ?: NONE }

        // Background and power restrictions.
        put("battery-optimisation") {
            val power = reads.power()
            "ignoring=${power.ignoringBatteryOptimisations}" +
                " power_save=${power.powerSave} idle=${power.deviceIdle}"
        }
        put("background-restricted") {
            val background = reads.background()
            "${background.restricted} low_ram=${background.lowRam}"
        }
        put("standby-bucket") { reads.standbyBucket().toString() }
        put("data-saver") {
            when (reads.restrictBackgroundStatus()) {
                ConnectivityManager.RESTRICT_BACKGROUND_STATUS_DISABLED -> "off"
                ConnectivityManager.RESTRICT_BACKGROUND_STATUS_WHITELISTED -> "on-whitelisted"
                ConnectivityManager.RESTRICT_BACKGROUND_STATUS_ENABLED -> "on"
                else -> "unknown"
            }
        }

        // VPN and tunnel.
        put("tunnel-state") { tunnelState }
        put("vpn-service-prepared") { reads.vpnServicePrepared().toString() }
        put("always-on") {
            val app = reads.secureSettingString("always_on_vpn_app")
            val lockdown = reads.secureSettingInt("always_on_vpn_lockdown")
            val word =
                when (app) {
                    null -> NONE
                    reads.packageName -> "this"
                    else -> "other"
                }
            "app=$word lockdown=${lockdown.settingWord()}"
        }
        put("other-vpn") {
            val activeIsVpn = reads.activeNetwork()?.transports?.contains(NetworkTransport.VPN) == true
            "active_is_vpn=$activeIsVpn"
        }

        // Network.
        put("network") { activeNetwork() }
        put("private-dns") { privateDns() }
        put("airplane-mode") { reads.globalSettingInt(Settings.Global.AIRPLANE_MODE_ON).settingWord() }

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
        val build = reads.build()
        val lineage =
            reads.systemProperty("ro.lineage.version")
                ?: reads.systemProperty("ro.lineage.build.version")
        val eos = E_OS_PACKAGES.any { reads.installedPackage(it) != null }
        val verdict =
            when {
                eos -> "e-os"
                lineage != null -> "lineage"
                reads.installedPackage(GRAPHENE_APPS) != null -> "graphene"
                build.tags?.contains("release-keys") == true -> "stock"
                else -> "other"
            }
        return "$verdict tags=${build.tags} type=${build.type} lineage=${lineage ?: NONE}"
    }

    private fun gmsVerdict(): String {
        val gms = reads.installedPackage(GMS)
        val microg = MICROG_PACKAGES.any { reads.installedPackage(it) != null }
        val vending = reads.installedPackage(VENDING) != null
        val version = gms?.versionName
        val kind =
            when {
                microg -> "microg"
                gms != null && version?.startsWith("0.") == true -> "microg"
                gms != null -> "google"
                else -> NONE
            }
        return "$kind vending=$vending version=${version ?: NONE}"
    }

    private fun deepLinkHandlers(): String {
        val handlers = reads.deepLinkHandlers(probeLink())
        val ours = reads.packageName in handlers
        return "ours=$ours count=${handlers.size} ${handlers.joinToString(",")}"
    }

    private fun deepLinkResolved(): String =
        when (reads.deepLinkResolvedPackage(probeLink())) {
            null -> NONE
            reads.packageName -> "this"
            "android" -> "chooser"
            else -> "other"
        }

    private fun defaultBrowser(): String {
        val pkg = reads.defaultBrowserPackage(BROWSER_PROBE_URL) ?: return NONE
        val version = reads.installedPackage(pkg)?.versionName
        return "$pkg ${version ?: ""}".trim()
    }

    /** Counts and classes only: the addresses and the resolvers stay on the device. */
    private fun activeNetwork(): String {
        val network = reads.activeNetwork() ?: return NONE
        val transports =
            NetworkTransport.entries.filter { it in network.transports }.joinToString("+") { it.word }
        val flags = NetworkFlag.entries.filter { it in network.flags }.joinToString(",") { it.word }
        val v4 = network.linkAddresses.count { ':' !in it }
        val v6 = network.linkAddresses.size - v4
        return "$transports [$flags] mtu=${network.mtu} v4=$v4 v6=$v6 dns=${network.dnsServers.size}"
    }

    private fun privateDns(): String {
        val network = reads.activeNetwork() ?: return "no-network"
        val mode = reads.globalSettingString("private_dns_mode") ?: "unset"
        return "active=${network.privateDnsActive} mode=$mode" +
            " named=${network.privateDnsServerName != null}"
    }

    companion object {
        private const val NONE = "none"
        private const val MS_PER_MINUTE = 60_000
        private const val MS_PER_SECOND = 1000
        private const val GMS = "com.google.android.gms"
        private const val VENDING = "com.android.vending"
        private const val GRAPHENE_APPS = "app.grapheneos.apps"
        private val E_OS_PACKAGES =
            listOf("foundation.e.apps", "foundation.e.browser", "foundation.e.blisslauncher")
        private val MICROG_PACKAGES = listOf("org.microg.gms", "com.mgoogle.android.gms")
        private const val BROWSER_PROBE_URL = "https://forum.warrenbrowse.com/"

        /**
         * The session id the deep-link probe carries: a placeholder, so the
         * intent resolution the header asks the system for never names a live
         * session (the resolver logs the URI it was asked about).
         */
        const val PROBE_SID = "00000000000000000000000000000000"

        /** The link the header asks the system to resolve, shaped as a real sign-in link. */
        fun probeLink(): String =
            "${BuildConfig.DEEP_LINK_SCHEME}://forum-login?sid=$PROBE_SID&host=connect.warrenbrowse.com"
    }
}
