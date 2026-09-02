package com.warrenbrowse.vpn.feature.settings.impl.support

import java.io.File
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertNotEquals
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.io.TempDir

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
    fun `the share sheet is handed a copy that outlives the report the screen deletes`(
        @TempDir dir: File
    ) {
        val report = File(dir, "warren-report-1.log").apply { writeText("redacted report") }

        val shared = sharedCopyOf(report.absolutePath)

        assertEquals(File(dir, "shared-warren-report-1.log"), shared)
        assertEquals("redacted report", shared.readText())
        // The screen discarding the report must leave the receiver's file.
        assertTrue(report.delete())
        assertTrue(shared.exists())
    }
}
