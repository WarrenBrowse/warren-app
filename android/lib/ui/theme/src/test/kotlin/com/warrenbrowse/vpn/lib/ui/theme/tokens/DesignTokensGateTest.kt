package com.warrenbrowse.vpn.lib.ui.theme.tokens

import java.io.File
import java.security.MessageDigest
import kotlin.test.assertEquals
import org.junit.jupiter.api.Test

/**
 * The JVM half of the design-token gate: `DesignTokens.kt` is generated from
 * `design-tokens.json` and carries the JSON's digest, so a JSON regenerated
 * without its Kotlin (or a Kotlin edited by hand) fails here. The vitest half
 * pins the JSON against the desktop sources.
 */
class DesignTokensGateTest {

    @Test
    fun `DesignTokens was generated from the checked-in design tokens json`() {
        val json = repoFile("design-tokens.json").readBytes()
        val digest =
            MessageDigest.getInstance("SHA-256").digest(json).joinToString("") { "%02x".format(it) }

        assertEquals(
            digest,
            DESIGN_TOKENS_SHA256,
            "DesignTokens.kt is stale: run `node scripts/design-tokens/gen.mjs` and commit both files",
        )
    }
}

/** The repo-root file of that name, found by walking up from the module directory Gradle runs in. */
internal fun repoFile(name: String): File =
    generateSequence(File("").absoluteFile) { it.parentFile }
        .map { File(it, name) }
        .first { it.isFile }
