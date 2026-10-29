package com.warrenbrowse.vpn.screen.devicerevoked

import com.warrenbrowse.vpn.lib.model.DeviceState
import kotlin.test.assertFalse
import kotlin.test.assertTrue
import org.junit.jupiter.api.Test

class DeviceRevokedGateTest {
    @Test
    fun `a revoked device is taken to the revoked screen from wherever it is`() {
        assertTrue(showsRevokedScreen(DeviceState.Revoked, revokedScreenInStack = false))
    }

    @Test
    fun `the revoked screen is not pushed again while it is in the stack, settings opened from it included`() {
        assertFalse(showsRevokedScreen(DeviceState.Revoked, revokedScreenInStack = true))
    }

    @Test
    fun `a device with no revocation stays where it is`() {
        assertFalse(showsRevokedScreen(null, revokedScreenInStack = false))
    }
}
