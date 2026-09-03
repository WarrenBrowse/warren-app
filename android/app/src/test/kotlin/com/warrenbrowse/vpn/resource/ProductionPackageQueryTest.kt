package com.warrenbrowse.vpn.resource

import com.warrenbrowse.vpn.app.product.PROD_APPLICATION_ID
import com.warrenbrowse.vpn.fixtures.ClientRulesFixtures
import com.warrenbrowse.vpn.fixtures.ClientRulesFixtures.string
import java.io.File
import javax.xml.parsers.DocumentBuilderFactory
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import kotlinx.serialization.json.jsonObject
import org.junit.jupiter.api.Test
import org.w3c.dom.Element

/**
 * Guards the package visibility a non-prod build needs to see the production
 * install at all. From Android 11 a `getPackageInfo` for a package the manifest
 * does not declare throws `NameNotFoundException`, exactly as it does for a
 * package that is genuinely absent, so a missing `<queries>` entry reports "no
 * production install" forever and the stand-down never fires. Nothing at
 * runtime can tell the two apart.
 *
 * The entry belongs to the non-prod flavor manifests: prod has no higher
 * environment to look for, and declaring it there would only widen prod's
 * visibility for nothing. That also means CI never compiles the flavor that
 * carries it (`testAllUnitTests` reaches `testProdDebugUnitTest` only), which
 * is why this test reads the manifests off the filesystem.
 */
class ProductionPackageQueryTest {

    @Test
    fun `the production application id is the one the fixture pins`() {
        // The constant is spelled in Kotlin because the lookup runs before any
        // native code, so the shared table cannot be asked at that point.
        val fixture =
            ClientRulesFixtures.load("product_env.json")["environments"]!!
                .jsonObject["prod"]!!
                .jsonObject
        assertEquals(fixture.string("application_id"), PROD_APPLICATION_ID)
    }

    @Test
    fun `every non-prod flavor queries the production package`() {
        val failures =
            NON_PROD_FLAVORS.mapNotNull { flavor ->
                if (PROD_APPLICATION_ID in queriedPackages(flavorManifest(flavor))) {
                    null
                } else {
                    "$flavor: no <queries> entry for $PROD_APPLICATION_ID"
                }
            }
        assertTrue(failures.isEmpty(), failures.joinToString("\n"))
    }

    @Test
    fun `the shared manifest declares no package query`() {
        // Prod merges the shared manifest too, and prod is not modified by this
        // campaign: an entry there would hand the production build a visibility
        // it has no use for.
        assertEquals(emptyList(), queriedPackages(File(androidRoot(), MAIN_MANIFEST)))
    }

    // Manifest access.

    private fun androidRoot(): File {
        var dir: File? = File("").absoluteFile
        while (dir != null) {
            if (File(dir, MAIN_MANIFEST).isFile) return dir
            dir = dir.parentFile
        }
        error("could not locate $MAIN_MANIFEST from ${File("").absolutePath}")
    }

    private fun flavorManifest(flavor: String): File {
        val manifest = File(androidRoot(), "app/src/$flavor/AndroidManifest.xml")
        assertTrue(manifest.isFile, "$flavor has no manifest at ${manifest.path}")
        return manifest
    }

    /** The package names a manifest declares inside `<queries>`, in document order. */
    private fun queriedPackages(manifest: File): List<String> {
        val root =
            DocumentBuilderFactory.newInstance().newDocumentBuilder().parse(manifest).documentElement
        val queries = root.getElementsByTagName("queries")
        return (0 until queries.length).flatMap { index ->
            val packages = (queries.item(index) as Element).getElementsByTagName("package")
            (0 until packages.length).map {
                (packages.item(it) as Element).getAttribute("android:name")
            }
        }
    }

    private companion object {
        const val MAIN_MANIFEST = "app/src/main/AndroidManifest.xml"

        /** Every flavor that has a higher-priority environment to stand down for. */
        val NON_PROD_FLAVORS = listOf("beta", "staging")
    }
}
