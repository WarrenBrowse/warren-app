package com.warrenbrowse.vpn.lib.ui.component

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Test

/** The beta label the home footer opens with (desktop `BetaCap`, mockup 8d): "BETA 20 Mbps". */
class BetaCapLabelTest {

    @Test
    fun `the label carries the live cap in whole megabits`() {
        assertEquals("BETA 20 Mbps", betaCapLabel("BETA", capBps = 20_000_000L))
    }

    @Test
    fun `the label stands alone while the cap is unknown or absent`() {
        assertEquals("BÊTA", betaCapLabel("BÊTA", capBps = null))
        assertEquals("BETA", betaCapLabel("BETA", capBps = 0L))
    }
}
