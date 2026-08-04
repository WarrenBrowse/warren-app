package com.warrenbrowse.vpn.feature.settings.impl

import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository
import kotlin.test.assertEquals
import org.junit.jupiter.api.Test

/**
 * Pure logic behind the MTU field's "Default" placeholder semantics (desktop
 * parity): the field is empty at the default so the placeholder shows, and
 * clearing it resets to the default (VpnService always needs a concrete MTU,
 * so there is no true "unset").
 */
class MtuFieldLogicTest {

    @Test
    fun `default mtu renders as empty so the Default placeholder shows`() {
        assertEquals("", mtuFieldDisplay(WarrenLocalSettingsRepository.MTU_MAX))
    }

    @Test
    fun `a non-default mtu renders as its number`() {
        assertEquals("1000", mtuFieldDisplay(1000))
        assertEquals(
            WarrenLocalSettingsRepository.MTU_MIN.toString(),
            mtuFieldDisplay(WarrenLocalSettingsRepository.MTU_MIN),
        )
    }

    @Test
    fun `blank input resets to the default mtu`() {
        assertEquals(WarrenLocalSettingsRepository.MTU_MAX, mtuFromInput(""))
    }

    @Test
    fun `numeric input is parsed`() {
        assertEquals(900, mtuFromInput("900"))
    }

    @Test
    fun `non-numeric input falls back to the default mtu`() {
        assertEquals(WarrenLocalSettingsRepository.MTU_MAX, mtuFromInput("abc"))
    }
}
