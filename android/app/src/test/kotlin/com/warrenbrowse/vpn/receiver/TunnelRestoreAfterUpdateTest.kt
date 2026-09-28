package com.warrenbrowse.vpn.receiver

import com.warrenbrowse.vpn.receiver.util.shouldRestoreTunnelAfterUpdate
import kotlin.test.assertFalse
import kotlin.test.assertTrue
import org.junit.jupiter.api.Test

class TunnelRestoreAfterUpdateTest {
    @Test
    fun `a tunnel that was up before the update comes back`() {
        assertTrue(shouldRestoreTunnelAfterUpdate(true, false, true))
    }

    @Test
    fun `a tunnel the user had turned off stays off`() {
        assertFalse(shouldRestoreTunnelAfterUpdate(false, false, true))
    }

    @Test
    fun `an always-on VPN is left to the OS, which restarts it by itself`() {
        assertFalse(shouldRestoreTunnelAfterUpdate(true, true, true))
    }

    @Test
    fun `nothing starts without the VPN permission`() {
        assertFalse(shouldRestoreTunnelAfterUpdate(true, false, false))
    }
}
