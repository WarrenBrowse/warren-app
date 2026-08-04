package com.warrenbrowse.vpn.resource

import java.io.File
import javax.xml.parsers.DocumentBuilderFactory
import kotlin.test.assertTrue
import org.junit.jupiter.api.Test
import org.w3c.dom.Element

/**
 * Guards the translated string resources against the ways they silently rot: a
 * key added to the default locale and never translated (57 of them had piled up
 * before this gate existed), a translation left behind when its English source
 * was reworded, and a translation whose placeholders no longer match the format
 * call, which throws at runtime rather than failing the build.
 *
 * It reads the resource files straight off the filesystem, so it stays a plain
 * JVM test in the `testAllUnitTests` set that CI runs.
 */
class StringResourceCoverageTest {

    @Test
    fun `every translatable key is present in every locale`() {
        val default = parse(defaultFile())
        val failures =
            locales().flatMap { (locale, file) ->
                val translated = parse(file).keys
                default.keys.filterNot { it in translated }.map { "$locale: missing $it" }
            }
        assertTrue(failures.isEmpty(), report("untranslated keys", failures))
    }

    @Test
    fun `no locale declares a key the default locale does not`() {
        // A key the default dropped is a translation of copy nobody shows any
        // more, and it hides the fact that the deletion was never propagated.
        val default = parse(defaultFile())
        val failures =
            locales().flatMap { (locale, file) ->
                parse(file).keys.filterNot { it in default.keys }.map { "$locale: unknown $it" }
            }
        assertTrue(failures.isEmpty(), report("stale keys", failures))
    }

    @Test
    fun `no resource file contains a long dash`() {
        val failures =
            (listOf("values" to defaultFile()) + locales() + nonTranslatableFiles()).flatMap {
                (locale, file) ->
                file.readLines().withIndex().filter { (_, line) ->
                    line.contains(EM_DASH) || line.contains(EN_DASH)
                }.map { (index, _) -> "$locale: line ${index + 1}" }
            }
        assertTrue(failures.isEmpty(), report("banned dashes", failures))
    }

    @Test
    fun `a translation never introduces a placeholder its source does not have`() {
        // Extra or renumbered placeholders throw at format time; using fewer than
        // the source is legitimate, because a language can drop a redundant count.
        val default = parse(defaultFile())
        val failures =
            locales().flatMap { (locale, file) ->
                parse(file).mapNotNull { (name, translated) ->
                    val source = default[name] ?: return@mapNotNull null
                    val extra = placeholders(translated) - placeholders(source)
                    if (extra.isEmpty()) {
                        null
                    } else {
                        "$locale: $name has ${extra.sorted()}," +
                            " source has ${placeholders(source).sorted()}"
                    }
                }
            }
        assertTrue(failures.isEmpty(), report("placeholder mismatches", failures))
    }

    @Test
    fun `every plural carries the quantities its locale needs`() {
        val failures =
            (listOf("values" to defaultFile()) + locales()).flatMap { (locale, file) ->
                val required = requiredQuantities(locale)
                parsePlurals(file).mapNotNull { (name, quantities) ->
                    val absent = required - quantities
                    if (absent.isEmpty()) null else "$locale: $name lacks ${absent.sorted()}"
                }
            }
        assertTrue(failures.isEmpty(), report("incomplete plurals", failures))
    }

    private fun report(what: String, failures: List<String>) =
        "${failures.size} $what:\n" + failures.joinToString("\n").take(MAX_REPORT_CHARS)

    // Resource access.

    private fun resDir(): File {
        var dir: File? = File("").absoluteFile
        while (dir != null) {
            val candidate = File(dir, RES_PATH)
            if (candidate.isDirectory) return candidate
            dir = dir.parentFile
        }
        error("could not locate $RES_PATH from ${File("").absolutePath}")
    }

    private fun defaultFile() = File(resDir(), "values/strings.xml")

    private fun nonTranslatableFiles() =
        listOf("values (non translatable)" to File(resDir(), "values/strings_non_translatable.xml"))

    /** Locale qualifier (`fr`, `zh-rCN`) to its strings.xml, sorted for stable output. */
    private fun locales(): List<Pair<String, File>> =
        resDir()
            .listFiles { file -> file.isDirectory && file.name.startsWith("values-") }
            .orEmpty()
            .map { it.name.removePrefix("values-") to File(it, "strings.xml") }
            .filter { (_, file) -> file.isFile }
            .sortedBy { (locale, _) -> locale }

    /** Translatable `<string>` entries, name to body. */
    private fun parse(file: File): Map<String, String> =
        elements(file, "string")
            .filterNot { it.getAttribute("translatable") == "false" }
            .associate { it.getAttribute("name") to it.textContent }

    /** `<plurals>` entries, name to the quantities it declares. */
    private fun parsePlurals(file: File): Map<String, Set<String>> =
        elements(file, "plurals").associate { plurals ->
            val items = plurals.getElementsByTagName("item")
            val quantities =
                (0 until items.length)
                    .map { (items.item(it) as Element).getAttribute("quantity") }
                    .toSet()
            plurals.getAttribute("name") to quantities
        }

    private fun elements(file: File, tag: String): List<Element> {
        val document =
            DocumentBuilderFactory.newInstance().newDocumentBuilder().parse(file).documentElement
        val nodes = document.getElementsByTagName(tag)
        return (0 until nodes.length).map { nodes.item(it) as Element }
    }

    // Rules.

    /**
     * A bare `%s` addresses argument 1 exactly like `%1$s`, so both forms have to
     * normalize to the same token or every single-argument string would read as a
     * mismatch.
     */
    private fun placeholders(body: String): Set<String> =
        PLACEHOLDER.findAll(body)
            .map { "%${it.groupValues[1].ifEmpty { "1" }}$${it.groupValues[2]}" }
            .toSet()

    private fun requiredQuantities(locale: String): Set<String> =
        when (locale.substringBefore('-')) {
            "ar" -> setOf("zero", "one", "two", "few", "many", "other")
            "pl", "ru", "uk" -> setOf("one", "few", "many", "other")
            "ro" -> setOf("one", "few", "other")
            // CLDR gives these languages a single form; Android still requires
            // "other" and ignores anything else.
            "ja", "ko", "my", "th", "zh" -> setOf("other")
            else -> setOf("one", "other")
        }

    private companion object {
        const val RES_PATH = "lib/ui/resource/src/main/res"
        const val MAX_REPORT_CHARS = 4000
        const val EM_DASH = '\u2014'
        const val EN_DASH = '\u2013'
        val PLACEHOLDER = Regex("""%(?:(\d+)\$)?([sd])""")
    }
}
