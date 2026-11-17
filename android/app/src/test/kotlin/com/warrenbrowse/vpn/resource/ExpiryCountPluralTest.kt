package com.warrenbrowse.vpn.resource

import java.io.File
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

/**
 * The expiry banner counts days, and a count only agrees with its noun in every language through
 * a `<plurals>` resource: a singular/plural pair picked in code is wrong for 2 and 3 days in
 * Polish, Romanian and Arabic. [StringResourceCoverageTest] then holds each locale to the
 * quantities its language needs.
 */
class ExpiryCountPluralTest {
    @Test
    fun `the days left in the expiry banner are a plural resource`() {
        val default = File(resDir(), "values/strings.xml").readText()

        assertTrue(default.contains("<plurals name=\"connect_subscription_expires_in\">"))
        assertFalse(default.contains("name=\"connect_subscription_expires_in_day\""))
        assertFalse(default.contains("name=\"connect_subscription_expires_in_days\""))
    }

    private fun resDir(): File {
        var dir: File? = File("").absoluteFile
        while (dir != null) {
            val candidate = File(dir, "lib/ui/resource/src/main/res")
            if (candidate.isDirectory) return candidate
            dir = dir.parentFile
        }
        error("no resource directory above ${File("").absolutePath}")
    }
}
