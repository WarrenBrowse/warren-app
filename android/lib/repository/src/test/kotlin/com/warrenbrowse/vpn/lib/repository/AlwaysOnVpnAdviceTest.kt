package com.warrenbrowse.vpn.lib.repository

import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

class AlwaysOnVpnAdviceTest {
    @Test
    fun `the advice is due while the system does not block connections without this VPN`() {
        assertTrue(alwaysOnVpnAdviceDue(alwaysOnVpn = false, lockdown = false, dismissed = false))
        // Always-on alone still lets traffic out while the tunnel is down.
        assertTrue(alwaysOnVpnAdviceDue(alwaysOnVpn = true, lockdown = false, dismissed = false))
    }

    @Test
    fun `the advice is not due once the system blocks connections without this VPN`() {
        assertFalse(alwaysOnVpnAdviceDue(alwaysOnVpn = true, lockdown = true, dismissed = false))
    }

    @Test
    fun `a dismissed advice is never shown again`() {
        assertFalse(alwaysOnVpnAdviceDue(alwaysOnVpn = false, lockdown = false, dismissed = true))
    }
}
