package com.warrenbrowse.vpn.feature.appinfo.impl.licenses

private const val APP_REPOSITORY = "https://github.com/WarrenBrowse/warren-app/"

// The tunnel engine (AGPL-3.0) the app is built against.
const val ENGINE_SOURCE_URL = "https://github.com/WarrenBrowse/warrenguard/"

private val RELEASE_VERSION = Regex("""^\d+\.\d+\.\d+$""")
private val HEADING = Regex("""^#+\s+""")
private val LIST_ITEM = Regex("""^-\s+""")

/**
 * The source of the installed version: a release is built from its tag, `v<version>` in production
 * and `beta-v<version>` in beta. Any other build links the repository itself.
 */
fun appSourceUrl(version: String, isBeta: Boolean): String =
    if (RELEASE_VERSION.matches(version)) {
        "${APP_REPOSITORY}tree/${if (isBeta) "beta-v" else "v"}$version"
    } else {
        APP_REPOSITORY
    }

/**
 * Reflows the licence for a narrow screen. The file is hard-wrapped Markdown: shown as is, every
 * wrapped line breaks again on the screen. Paragraphs and list items become single lines, headings
 * lose their markers, and blank lines stay paragraph breaks. Same rule as the desktop app's.
 */
fun formatLicenseText(markdown: String): String =
    markdown
        .split(Regex("""\n\s*\n"""))
        .map { block ->
            val lines = mutableListOf<String>()
            for (raw in block.split("\n")) {
                val line = raw.trim()
                if (line.isEmpty()) continue
                if (HEADING.containsMatchIn(line) || LIST_ITEM.containsMatchIn(line) || lines.isEmpty()) {
                    lines.add(line.replace(HEADING, "").replace(LIST_ITEM, ""))
                } else {
                    lines[lines.lastIndex] = "${lines.last()} $line"
                }
            }
            lines.joinToString("\n")
        }
        .filter { it.isNotEmpty() }
        .joinToString("\n\n")
