package com.warrenbrowse.vpn.app.forum

import java.io.File
import javax.xml.parsers.DocumentBuilderFactory
import kotlin.test.assertTrue
import org.junit.jupiter.api.Test
import org.w3c.dom.Element

/**
 * The cross-device consent prompt is raised for two inputs that share nothing
 * but their uncertainty: a deep link carrying `xd=1` (the QR on the approval
 * page) and a sign-in code typed under Settings, which arrives with no link
 * and no signal at all. So its copy must not tell the reader where the request
 * came from: a typed code has no QR anywhere in the flow, and a prompt that
 * names one reads as a confused app precisely when it is asking the reader to
 * weigh a relayed-approval attack.
 */
class ForumLoginPromptCopyTest {

    @Test
    fun the_cross_device_prompt_never_names_a_qr_code_in_any_locale() {
        val failures =
            localeFiles().flatMap { (locale, file) ->
                val strings = parse(file)
                CROSS_DEVICE_KEYS.flatMap { key ->
                    val body = strings[key] ?: return@flatMap emptyList<String>()
                    QR_TOKENS.filter { body.contains(it, ignoreCase = true) }
                        .map { "$locale: $key names \"$it\"" }
                }
            }
        assertTrue(
            failures.isEmpty(),
            "${failures.size} cross-device strings claim a QR origin:\n" + failures.joinToString("\n"),
        )
    }

    private fun localeFiles(): List<Pair<String, File>> {
        val res = resDir()
        val default = listOf("values" to File(res, "values/strings.xml"))
        val translated =
            res.listFiles { file -> file.isDirectory && file.name.startsWith("values-") }
                .orEmpty()
                .map { it.name to File(it, "strings.xml") }
                .filter { (_, file) -> file.isFile }
                .sortedBy { (locale, _) -> locale }
        return default + translated
    }

    private fun resDir(): File {
        var dir: File? = File("").absoluteFile
        while (dir != null) {
            val candidate = File(dir, RES_PATH)
            if (candidate.isDirectory) return candidate
            dir = dir.parentFile
        }
        error("could not locate $RES_PATH from ${File("").absolutePath}")
    }

    private fun parse(file: File): Map<String, String> {
        val document =
            DocumentBuilderFactory.newInstance().newDocumentBuilder().parse(file).documentElement
        val nodes = document.getElementsByTagName("string")
        return (0 until nodes.length)
            .map { nodes.item(it) as Element }
            .associate { it.getAttribute("name") to it.textContent }
    }

    private companion object {
        const val RES_PATH = "lib/ui/resource/src/main/res"
        val CROSS_DEVICE_KEYS =
            listOf(
                "forum_login_title_cross_device",
                "forum_login_body_first_cross_device",
                "forum_login_body_second_cross_device",
            )

        /**
         * "QR" survives untranslated in most locales; Simplified Chinese and Thai
         * spell it out, so both spellings are listed rather than assuming the
         * Latin token covers every language.
         */
        val QR_TOKENS = listOf("QR", "二维码", "二維碼", "คิวอาร์")
    }
}
