package com.warrenbrowse.vpn.app.forum

import java.io.File
import javax.xml.parsers.DocumentBuilderFactory
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Test
import org.w3c.dom.Element

/**
 * The share sheet's `FileProvider` serves the directory `res/xml/report_paths.xml`
 * names, and the reporter writes to `REPORT_DIR`. They are two spellings of one
 * contract with no compile-time link: a rename of either leaves every other test
 * green and makes `FileProvider.getUriForFile` throw inside the share tap.
 */
class ReportProviderPathsTest {

    @Test
    fun the_provider_serves_exactly_the_directory_the_reporter_writes_to() {
        val paths = parse(File(resDir(), "xml/report_paths.xml"))
        val cachePaths = paths.getElementsByTagName("cache-path")
        assertEquals(1, cachePaths.length, "one root, the report directory")
        val root = cachePaths.item(0) as Element
        assertEquals("${WarrenSupportReporterImpl.REPORT_DIR}/", root.getAttribute("path"))
        assertEquals(0, paths.getElementsByTagName("files-path").length)
        assertEquals(0, paths.getElementsByTagName("external-path").length)
        assertEquals(0, paths.getElementsByTagName("root-path").length)
    }

    private fun parse(file: File): Element =
        DocumentBuilderFactory.newInstance().newDocumentBuilder().parse(file).documentElement

    /** Gradle runs the tests from the module directory; a plain JVM runner may not. */
    private fun resDir(): File {
        var dir: File? = File("").absoluteFile
        while (dir != null) {
            val candidate = File(dir, RES_PATH)
            if (candidate.isDirectory) return candidate
            dir = dir.parentFile
        }
        error("could not locate $RES_PATH from ${File("").absolutePath}")
    }

    private companion object {
        const val RES_PATH = "android/app/src/main/res"
    }
}
