package com.warrenbrowse.vpn.resource

import java.io.File
import javax.xml.parsers.DocumentBuilderFactory
import kotlin.test.assertTrue
import org.junit.jupiter.api.Test
import org.w3c.dom.Element

/**
 * Guards the beta marker on the status-bar surfaces: the ongoing tunnel
 * notification, the forum notification and the quick-settings tile all draw the
 * bare Warren mark, and a device carrying both installs would otherwise show two
 * identical icons with no way to tell which product is holding the tunnel.
 *
 * Android flattens a notification small icon to an alpha mask and tints it, so
 * the marker has to be a shape rather than a colour: the amber pip carries the
 * `B` as a knocked-out hole, the same way the desktop tray assets do.
 *
 * The marker is a resource overlay in the non-prod flavors, so no call site
 * names it and nothing selects it at runtime. That also means the beta flavor is
 * never compiled by CI (`testAllUnitTests` reaches `testProdDebugUnitTest`
 * only), which is why this test reads both trees off the filesystem instead of
 * resolving an `R.drawable` id.
 */
class BetaSmallLogoOverrideTest {

    @Test
    fun `every status bar drawable is overridden in every non-prod flavor`() {
        val drawables = statusBarDrawables()
        assertTrue(drawables.isNotEmpty(), "no status-bar drawable found, the source scan is broken")
        val failures =
            BADGED_FLAVORS.flatMap { flavor ->
                drawables
                    .filterNot { overlayFile(flavor, it).isFile }
                    .map { "$flavor: no override for $it" }
            }
        assertTrue(failures.isEmpty(), report("unbadged status-bar drawables", failures))
    }

    @Test
    fun `an override renders at the size of the drawable it replaces`() {
        // A different viewport or intrinsic size would move the mark inside the
        // status bar rather than badge it.
        val failures =
            eachOverride { flavor, name, prod, override ->
                SIZE_ATTRIBUTES.mapNotNull { attribute ->
                    val expected = prod.getAttribute(attribute)
                    val actual = override.getAttribute(attribute)
                    if (expected == actual) {
                        null
                    } else {
                        "$flavor/$name: $attribute is '$actual', prod has '$expected'"
                    }
                }
            }
        assertTrue(failures.isEmpty(), report("resized overrides", failures))
    }

    @Test
    fun `an override is not a copy of the drawable it replaces`() {
        // An overlay that merely duplicates the prod artwork ships a beta build
        // whose notification is indistinguishable from prod's, and nothing else
        // would notice: the resource resolves, the icon draws.
        val failures =
            eachOverride { flavor, name, prod, override ->
                if (pathData(override) == pathData(prod)) {
                    listOf("$flavor/$name: carries the prod path data unchanged")
                } else {
                    emptyList()
                }
            }
        assertTrue(failures.isEmpty(), report("unbadged copies", failures))
    }

    @Test
    fun `an override still carries the mark of the drawable it replaces`() {
        // The overlay is a copy of the mark plus a pip, so a redraw of the mark
        // in the library leaves the badged copies showing the old logo. Pinning
        // the mark path here turns that silent drift into a red test.
        val failures =
            eachOverride { flavor, name, prod, override ->
                val badged = pathData(override)
                pathData(prod)
                    .filterNot { it in badged }
                    .map { "$flavor/$name: the prod mark path is absent, regenerate the overlay" }
            }
        assertTrue(failures.isEmpty(), report("stale overrides", failures))
    }

    @Test
    fun `the non-prod flavors share one badged artwork`() {
        // Desktop generates a single non-prod tray tree and serves it to
        // every non-prod environment. Two hand-kept copies here would drift.
        val reference = BADGED_FLAVORS.first()
        val failures =
            statusBarDrawables().flatMap { name ->
                val expected = overlayFile(reference, name).readText()
                BADGED_FLAVORS.drop(1)
                    .filterNot { overlayFile(it, name).readText() == expected }
                    .map { "$it/$name: differs from $reference" }
            }
        assertTrue(failures.isEmpty(), report("diverging overrides", failures))
    }

    // Resource access.

    private fun eachOverride(
        check: (flavor: String, name: String, prod: Element, override: Element) -> List<String>
    ): List<String> =
        BADGED_FLAVORS.flatMap { flavor ->
            statusBarDrawables().flatMap { name ->
                check(flavor, name, vector(prodFile(name)), vector(overlayFile(flavor, name)))
            }
        }

    private fun androidRoot(): File {
        var dir: File? = File("").absoluteFile
        while (dir != null) {
            if (File(dir, PROD_DRAWABLE_PATH).isDirectory) return dir
            dir = dir.parentFile
        }
        error("could not locate $PROD_DRAWABLE_PATH from ${File("").absolutePath}")
    }

    private fun prodFile(name: String) = File(androidRoot(), "$PROD_DRAWABLE_PATH/$name.xml")

    private fun overlayFile(flavor: String, name: String) =
        File(androidRoot(), "app/src/$flavor/res/drawable/$name.xml")

    /** The drawables the status bar and the quick-settings tile actually draw. */
    private fun statusBarDrawables(): Set<String> {
        val notification =
            kotlinSources("lib/push-notification/src/main/kotlin").flatMap {
                NOTIFICATION_SMALL_ICON.findAll(it.readText()).map { match -> match.groupValues[1] }
            }
        val tile =
            kotlinSources("app/src/main/kotlin/com/warrenbrowse/vpn/app/tile").flatMap {
                TILE_ICON.findAll(it.readText()).map { match -> match.groupValues[1] }
            }
        val manifest =
            MANIFEST_ICON
                .findAll(File(androidRoot(), MANIFEST_PATH).readText())
                .map { it.groupValues[1] }
        return (notification + tile + manifest).toSet()
    }

    private fun kotlinSources(path: String): List<File> =
        File(androidRoot(), path).walkTopDown().filter { it.extension == "kt" }.toList()

    private fun vector(file: File): Element {
        val root =
            DocumentBuilderFactory.newInstance().newDocumentBuilder().parse(file).documentElement
        check(root.tagName == "vector") { "${file.path} is a <${root.tagName}>, not a vector" }
        return root
    }

    /** Every `<path>` body of a vector drawable, in document order. */
    private fun pathData(vector: Element): List<String> {
        val paths = vector.getElementsByTagName("path")
        return (0 until paths.length).map {
            (paths.item(it) as Element).getAttribute("android:pathData")
        }
    }

    private fun report(what: String, failures: List<String>) =
        "${failures.size} $what:\n" + failures.joinToString("\n").take(MAX_REPORT_CHARS)

    private companion object {
        const val PROD_DRAWABLE_PATH = "lib/ui/resource/src/main/res/drawable"
        const val MANIFEST_PATH = "app/src/main/AndroidManifest.xml"
        const val MAX_REPORT_CHARS = 4000

        /** Every flavor whose install has to be tellable from prod on one device. */
        val BADGED_FLAVORS = listOf("beta", "staging")

        val SIZE_ATTRIBUTES =
            listOf(
                "android:width",
                "android:height",
                "android:viewportWidth",
                "android:viewportHeight",
            )

        val NOTIFICATION_SMALL_ICON = Regex("""setSmallIcon\(R\.drawable\.(\w+)\)""")
        val TILE_ICON = Regex("""Icon\.createWithResource\([^)]*R\.drawable\.(\w+)\)""")
        val MANIFEST_ICON = Regex("""android:icon="@drawable/(\w+)"""")
    }
}
