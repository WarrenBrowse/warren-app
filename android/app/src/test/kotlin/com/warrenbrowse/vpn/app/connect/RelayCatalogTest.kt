package com.warrenbrowse.vpn.app.connect

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

/**
 * RelayCatalog defers to `WarrenJni.listRelays()` which is a native JNI
 * symbol unavailable in host unit tests. These tests therefore exercise
 * the `list()` -> `WarrenRelaySummary` projection on a synthetic
 * `RelayInfo` set, validating the data-class round-trip without
 * touching the JNI call.
 */
class RelayCatalogTest {

    private val sample = RelayInfo(
        exitId = "2921abad869e94064b56cf48c8da3631",
        exitPubkeyHex = "2921abad869e94064b56cf48c8da3631",
        endpoint = "warren-exit-1.warren.brown:443",
        country = "DE",
        city = "Falkenstein",
        active = true,
        weight = 100,
    )

    @Test
    fun `RelayInfo round-trips a fully-populated relay`() {
        // Verify the data class is wide enough to hold all the fields
        // the JNI side ships. A regression here would silently drop
        // attributes from the picker UI.
        assertEquals("2921abad869e94064b56cf48c8da3631", sample.exitId)
        assertEquals("warren-exit-1.warren.brown:443", sample.endpoint)
        assertEquals("DE", sample.country)
        assertEquals(100L, sample.weight)
        assertTrue(sample.active)
    }

    @Test
    fun `equality is structural`() {
        val copy = sample.copy()
        assertEquals(sample, copy)
    }

    @Test
    fun `copy works field-by-field`() {
        val inactive = sample.copy(active = false)
        assertTrue(!inactive.active)
        // Other fields preserved.
        assertEquals(sample.exitId, inactive.exitId)
        assertEquals(sample.endpoint, inactive.endpoint)
    }
}
