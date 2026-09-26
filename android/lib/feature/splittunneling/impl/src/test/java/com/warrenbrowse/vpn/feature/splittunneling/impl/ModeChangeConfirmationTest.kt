package com.warrenbrowse.vpn.feature.splittunneling.impl

import kotlin.test.assertEquals
import kotlin.test.assertNull
import com.warrenbrowse.vpn.lib.model.SplitTunnelMode
import org.junit.jupiter.api.Test

/** Mirrors the desktop `modeChangeConfirmation` (shared/app-routing.ts). */
class ModeChangeConfirmationTest {

    @Test
    fun `turning vpn only for on from off warns that the rest of the device is unprotected`() {
        assertEquals(
            ModeChangeConfirmation(leavesDeviceUnprotected = true, replaces = null),
            modeChangeConfirmation(SplitTunnelMode.Off, SplitTunnelMode.IncludeOnly),
        )
    }

    @Test
    fun `turning vpn only for on over bypass also says bypass is replaced`() {
        assertEquals(
            ModeChangeConfirmation(leavesDeviceUnprotected = true, replaces = SplitTunnelMode.Exclude),
            modeChangeConfirmation(SplitTunnelMode.Exclude, SplitTunnelMode.IncludeOnly),
        )
    }

    @Test
    fun `turning bypass on over vpn only for says vpn only for is replaced`() {
        assertEquals(
            ModeChangeConfirmation(
                leavesDeviceUnprotected = false,
                replaces = SplitTunnelMode.IncludeOnly,
            ),
            modeChangeConfirmation(SplitTunnelMode.IncludeOnly, SplitTunnelMode.Exclude),
        )
    }

    @Test
    fun `turning bypass on from off needs no confirmation`() {
        assertNull(modeChangeConfirmation(SplitTunnelMode.Off, SplitTunnelMode.Exclude))
    }

    @Test
    fun `turning a mode off or keeping it needs no confirmation`() {
        assertNull(modeChangeConfirmation(SplitTunnelMode.IncludeOnly, SplitTunnelMode.Off))
        assertNull(modeChangeConfirmation(SplitTunnelMode.IncludeOnly, SplitTunnelMode.IncludeOnly))
    }
}
