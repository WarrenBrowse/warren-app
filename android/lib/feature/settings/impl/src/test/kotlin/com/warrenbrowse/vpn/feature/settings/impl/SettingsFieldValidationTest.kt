package com.warrenbrowse.vpn.feature.settings.impl

import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue
import org.junit.jupiter.api.Test

/**
 * Commit rules for the numeric settings fields (desktop parity): a field is
 * only persisted when the typed value is inside the range the hint states, and
 * a blank field means the documented "no override" value rather than zero.
 */
class SettingsFieldValidationTest {

    @Test
    fun `mtu accepts the stated range and rejects outside it`() {
        assertTrue(mtuInputIsValid(SettingsRanges.MTU_MIN.toString()))
        assertTrue(mtuInputIsValid(SettingsRanges.MTU_MAX.toString()))
        assertTrue(mtuInputIsValid("1000"))
        assertFalse(mtuInputIsValid((SettingsRanges.MTU_MIN - 1).toString()))
        assertFalse(mtuInputIsValid((SettingsRanges.MTU_MAX + 1).toString()))
    }

    @Test
    fun `a blank mtu is valid and means the default`() {
        assertTrue(mtuInputIsValid(""))
        assertEquals(SettingsRanges.MTU_MAX, mtuFromInput(""))
    }

    @Test
    fun `max bandwidth accepts the stated range and rejects outside it`() {
        assertTrue(maxRateInputIsValid(SettingsRanges.MAX_RATE_MBPS_MIN.toString()))
        assertTrue(maxRateInputIsValid(SettingsRanges.MAX_RATE_MBPS_MAX.toString()))
        assertFalse(maxRateInputIsValid("0"))
        assertFalse(maxRateInputIsValid((SettingsRanges.MAX_RATE_MBPS_MAX + 1).toString()))
    }

    @Test
    fun `a blank max bandwidth is valid and means unlimited`() {
        assertTrue(maxRateInputIsValid(""))
        assertEquals(0L, maxRateBpsFromInput(""))
    }

    @Test
    fun `the preferred port accepts only the range the exit allocator can honour`() {
        assertTrue(natPmpPortInputIsValid(SettingsRanges.NATPMP_PORT_MIN.toString()))
        assertTrue(natPmpPortInputIsValid(SettingsRanges.NATPMP_PORT_MAX.toString()))
        assertFalse(natPmpPortInputIsValid("80"))
        assertFalse(natPmpPortInputIsValid("8080"))
        assertFalse(natPmpPortInputIsValid((SettingsRanges.NATPMP_PORT_MIN - 1).toString()))
        assertFalse(natPmpPortInputIsValid("65536"))
    }

    @Test
    fun `a blank preferred port is valid and lets the exit choose`() {
        assertTrue(natPmpPortInputIsValid(""))
        assertEquals(0, natPmpPortFromInput(""))
        assertEquals(50000, natPmpPortFromInput("50000"))
    }
}
