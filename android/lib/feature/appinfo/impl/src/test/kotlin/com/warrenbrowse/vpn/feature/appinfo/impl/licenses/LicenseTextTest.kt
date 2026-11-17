package com.warrenbrowse.vpn.feature.appinfo.impl.licenses

import java.io.File
import kotlin.test.assertEquals
import org.junit.jupiter.api.Test

class LicenseTextTest {

    @Test
    fun `a beta release links the source at its beta tag`() {
        assertEquals(
            "https://github.com/WarrenBrowse/warren-app/tree/beta-v1.1.57",
            appSourceUrl(version = "1.1.57", isBeta = true),
        )
    }

    @Test
    fun `a production release links the source at its tag`() {
        assertEquals(
            "https://github.com/WarrenBrowse/warren-app/tree/v1.2.0",
            appSourceUrl(version = "1.2.0", isBeta = false),
        )
    }

    @Test
    fun `a build that is no release links the repository`() {
        assertEquals(
            "https://github.com/WarrenBrowse/warren-app/",
            appSourceUrl(version = "1.1.57-dev-0123abcd", isBeta = true),
        )
    }

    @Test
    fun `the licence is reflowed for a narrow screen`() {
        val markdown =
            listOf(
                    "### GNU GENERAL PUBLIC LICENSE",
                    "",
                    "The licenses for most software are designed",
                    "to take away your freedom.",
                    "",
                    "-   a) The work must carry prominent notices stating that you modified",
                    "    it, and giving a relevant date.",
                    "-   b) The work must carry prominent notices.",
                )
                .joinToString("\n")

        assertEquals(
            listOf(
                    "GNU GENERAL PUBLIC LICENSE",
                    "",
                    "The licenses for most software are designed to take away your freedom.",
                    "",
                    "a) The work must carry prominent notices stating that you modified it, " +
                        "and giving a relevant date.",
                    "b) The work must carry prominent notices.",
                )
                .joinToString("\n"),
            formatLicenseText(markdown),
        )
    }

    // The APK carries its own copy of the licence: it must stay the repository's.
    @Test
    fun `the packaged licence is the repository licence`() {
        val repositoryLicense = File("../../../../../LICENSE.md")
        val packagedLicense = File("../../../ui/resource/src/main/res/raw/gpl_3_0.md")

        assertEquals(repositoryLicense.readText(), packagedLicense.readText())
    }
}
