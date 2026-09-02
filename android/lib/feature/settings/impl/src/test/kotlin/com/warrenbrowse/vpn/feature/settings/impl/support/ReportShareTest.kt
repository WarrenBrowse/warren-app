package com.warrenbrowse.vpn.feature.settings.impl.support

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertNotEquals
import org.junit.jupiter.api.Test

class ReportShareTest {

    @Test
    fun `the provider authority follows the running package, never the prod id`() {
        assertEquals("com.warrenbrowse.vpn.beta.reports", reportProviderAuthority("com.warrenbrowse.vpn.beta"))
        assertEquals(
            "com.warrenbrowse.vpn.staging.reports",
            reportProviderAuthority("com.warrenbrowse.vpn.staging"),
        )
        assertEquals("com.warrenbrowse.vpn.reports", reportProviderAuthority("com.warrenbrowse.vpn"))
        assertNotEquals(
            reportProviderAuthority("com.warrenbrowse.vpn"),
            reportProviderAuthority("com.warrenbrowse.vpn.beta"),
            "two flavors installed side by side must not claim one authority",
        )
    }

    @Test
    fun `a shared report is offered as plain text`() {
        assertEquals("text/plain", REPORT_MIME_TYPE)
    }
}
