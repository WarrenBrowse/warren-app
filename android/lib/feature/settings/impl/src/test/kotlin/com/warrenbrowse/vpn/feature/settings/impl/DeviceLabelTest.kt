package com.warrenbrowse.vpn.feature.settings.impl

import kotlin.test.assertEquals
import kotlin.test.assertTrue
import org.junit.jupiter.api.Test

class DeviceLabelTest {

    @Test
    fun `zero or negative timestamp renders unknown date`() {
        assertEquals("unknown date", deviceCreatedLabel(0L))
        assertEquals("unknown date", deviceCreatedLabel(-5L))
    }

    @Test
    fun `positive timestamp renders an ISO local date`() {
        // 1700000000 = 2023-11-14/15 depending on zone; assert ISO shape.
        val label = deviceCreatedLabel(1_700_000_000L)
        assertTrue(label.matches(Regex("""\d{4}-\d{2}-\d{2}""")), label)
    }
}
