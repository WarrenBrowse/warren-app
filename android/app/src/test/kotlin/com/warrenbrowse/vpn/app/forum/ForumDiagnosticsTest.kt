package com.warrenbrowse.vpn.app.forum

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

class ForumDiagnosticsTest {

    private fun collect(reads: FakePlatformReads, lastLogin: String? = null): Map<String, String> =
        ForumDiagnostics(reads).collect("Connected", "locked", lastLogin)

    @Test
    fun the_header_keys_are_pinned_in_order() {
        val expected =
            listOf(
                "report-schema",
                "warren-product-env",
                "android-application-id",
                "android-build-type",
                "deep-link-scheme",
                "installer-package",
                "android-fingerprint",
                "android-rom",
                "gms",
                "webview-package",
                "time-now-utc",
                "time-zone",
                "time-auto",
                "uptime",
                "locale",
                "deep-link-handlers",
                "deep-link-resolved",
                "default-browser",
                "last-forum-login",
                "battery-optimisation",
                "background-restricted",
                "standby-bucket",
                "data-saver",
                "tunnel-state",
                "vpn-service-prepared",
                "always-on",
                "other-vpn",
                "network",
                "private-dns",
                "airplane-mode",
                "wallet",
            )

        assertEquals(expected, collect(FakePlatformReads()).keys.toList())
    }

    @Test
    fun a_device_without_a_webview_reads_none() {
        val reads = FakePlatformReads().apply { webView = null }

        assertEquals("none", collect(reads)["webview-package"])
    }

    @Test
    fun a_throwing_read_costs_only_its_key() {
        // Usage stats are the read a locked-down ROM refuses first.
        val reads =
            FakePlatformReads().apply { failing["standbyBucket"] = SecurityException("usage stats") }

        val facts = collect(reads)

        assertEquals("unreadable:SecurityException", facts["standby-bucket"])
        assertEquals("off", facts["data-saver"])
        assertEquals("com.google.android.webview 120.0.6099.230", facts["webview-package"])
        val unreadable = facts.filterValues { it.startsWith("unreadable") }.keys
        assertEquals(setOf("standby-bucket"), unreadable)
    }

    @Test
    fun no_value_carries_an_address_a_resolver_an_ssid_or_a_private_dns_name() {
        val markers =
            listOf("192.0.2.77", "2001:db8::77", "198.51.100.53", "SSID-MARKER", "PRIVATE-DNS-MARKER")
        val reads =
            FakePlatformReads().apply {
                network =
                    network!!.copy(
                        linkAddresses = listOf("192.0.2.77", "2001:db8::77"),
                        dnsServers = listOf("198.51.100.53"),
                        ssid = "SSID-MARKER",
                        privateDnsActive = true,
                        privateDnsServerName = "PRIVATE-DNS-MARKER",
                    )
            }

        val facts = collect(reads)

        for ((key, value) in facts) {
            for (marker in markers) {
                assertFalse(value.contains(marker), "$key carries $marker: $value")
            }
        }
        assertEquals("wifi+vpn [validated,internet] mtu=1280 v4=1 v6=1 dns=1", facts["network"])
        assertEquals("active=true mode=opportunistic named=true", facts["private-dns"])
    }

    @Test
    fun the_deep_link_probe_never_carries_a_live_session_id() {
        val reads = FakePlatformReads()

        collect(reads)

        assertEquals(2, reads.probedLinks.size)
        assertTrue(reads.probedLinks.all { it == ForumDiagnostics.probeLink() })
        assertTrue(ForumDiagnostics.probeLink().contains("sid=${ForumDiagnostics.PROBE_SID}"))
    }

    @Test
    fun the_deep_link_lines_name_this_app_by_role_and_the_handlers_by_package() {
        val reads =
            FakePlatformReads().apply {
                handlers = listOf(packageName, "org.mozilla.firefox")
                resolved = "android"
            }

        val facts = collect(reads)

        assertEquals("ours=true count=2 com.warrenbrowse.vpn.beta,org.mozilla.firefox", facts["deep-link-handlers"])
        assertEquals("chooser", facts["deep-link-resolved"])
        assertEquals("org.mozilla.firefox 154.0.1", facts["default-browser"])
    }

    @Test
    fun a_lineage_rom_is_named_by_its_property() {
        val reads =
            FakePlatformReads().apply {
                properties["ro.lineage.version"] = "21.0-20240105-NIGHTLY-panther"
                buildFacts = buildFacts.copy(tags = "test-keys", type = "userdebug")
            }

        assertEquals(
            "lineage tags=test-keys type=userdebug lineage=21.0-20240105-NIGHTLY-panther",
            collect(reads)["android-rom"],
        )
    }

    @Test
    fun microg_is_told_from_google_services_by_its_version() {
        val reads = FakePlatformReads().apply { packages["com.google.android.gms"] = "0.3.1.4" }

        assertEquals("microg vending=true version=0.3.1.4", collect(reads)["gms"])
    }

    @Test
    fun a_device_without_google_services_reads_none() {
        val reads =
            FakePlatformReads().apply {
                packages.remove("com.google.android.gms")
                packages.remove("com.android.vending")
            }

        assertEquals("none vending=false version=none", collect(reads)["gms"])
    }

    @Test
    fun an_unset_setting_reads_unreadable() {
        val reads = FakePlatformReads().apply { globalInts.remove("auto_time") }

        assertEquals("auto_time=unreadable auto_time_zone=1", collect(reads)["time-auto"])
    }

    @Test
    fun always_on_names_this_app_by_role_not_by_package() {
        val ours = FakePlatformReads()
        val other = FakePlatformReads().apply { secureStrings["always_on_vpn_app"] = "com.example.othervpn" }
        val none = FakePlatformReads().apply { secureStrings.remove("always_on_vpn_app") }

        assertEquals("app=this lockdown=1", collect(ours)["always-on"])
        assertEquals("app=other lockdown=1", collect(other)["always-on"])
        assertEquals("app=none lockdown=1", collect(none)["always-on"])
    }

    @Test
    fun the_inputs_land_in_their_keys() {
        val facts = ForumDiagnostics(FakePlatformReads()).collect("Reconnecting", "absent", "expired")

        assertEquals("Reconnecting", facts["tunnel-state"])
        assertEquals("absent", facts["wallet"])
        assertEquals("expired", facts["last-forum-login"])
        assertEquals("none", collect(FakePlatformReads())["last-forum-login"])
    }

    @Test
    fun the_clock_lines_carry_the_zone_offset_and_the_uptime_in_whole_units() {
        val facts = collect(FakePlatformReads())

        assertEquals("2026-09-02T21:38:46Z", facts["time-now-utc"])
        assertEquals("Europe/Paris offset=60min", facts["time-zone"])
        assertEquals("elapsed=90s process_age=60s", facts["uptime"])
        assertEquals("fr-FR", facts["locale"])
    }
}
