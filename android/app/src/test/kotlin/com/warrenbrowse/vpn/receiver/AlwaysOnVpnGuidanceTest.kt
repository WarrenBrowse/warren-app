package com.warrenbrowse.vpn.receiver

import com.warrenbrowse.vpn.receiver.util.AlwaysOnVpnGuidance
import com.warrenbrowse.vpn.receiver.util.alwaysOnVpnGuidance
import kotlin.test.assertEquals
import org.junit.jupiter.api.Test

class AlwaysOnVpnGuidanceTest {
    private val ours = "com.warrenbrowse.vpn"

    @Test
    fun `always-on with lockdown for this app needs no notice`() {
        assertEquals(
            AlwaysOnVpnGuidance.CONFIGURED,
            alwaysOnVpnGuidance(ours, true, ours),
        )
    }

    @Test
    fun `always-on without the blocking flag still lets traffic out during a replacement`() {
        assertEquals(
            AlwaysOnVpnGuidance.NOT_CONFIGURED,
            alwaysOnVpnGuidance(ours, false, ours),
        )
    }

    @Test
    fun `always-on pointed at another app protects nothing of ours`() {
        assertEquals(
            AlwaysOnVpnGuidance.NOT_CONFIGURED,
            alwaysOnVpnGuidance("com.example.other", true, ours),
        )
    }

    @Test
    fun `a device that hides the settings is never warned`() {
        // These keys are not public API. Warning on an unreadable device would
        // tell a correctly configured user they are unprotected, which is worse
        // than saying nothing.
        assertEquals(
            AlwaysOnVpnGuidance.UNKNOWN,
            alwaysOnVpnGuidance(null, null, ours),
        )
    }
}
