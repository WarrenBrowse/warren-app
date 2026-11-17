package com.warrenbrowse.vpn.feature.login.impl

import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue
import org.junit.jupiter.api.Test

// Ticking "I have written it down" proves nothing: the check asks for words only someone holding
// the phrase can give, before the account is used. Same rule as the desktop app.
class BackupCheckTest {

    private val phrase =
        "legal winner thank year wave sausage worth useful legal winner thank yellow"

    private fun sequence(vararg values: Double): () -> Double {
        var index = 0
        return { values[index++ % values.size] }
    }

    @Test
    fun `asks for three distinct positions of the phrase, in order`() {
        // 0.99 twice draws the last position twice: the duplicate is redrawn.
        assertEquals(
            listOf(0, 6, 11),
            pickBackupCheckPositions(12, sequence(0.99, 0.99, 0.0, 0.5)),
        )
    }

    @Test
    fun `passes when every asked word is given, whatever the case and spacing`() {
        assertTrue(backupCheckPasses(phrase, listOf(0, 6, 11), listOf("Legal ", " worth", "YELLOW")))
    }

    @Test
    fun `fails when a word is wrong`() {
        assertFalse(backupCheckPasses(phrase, listOf(0, 6, 11), listOf("legal", "worth", "year")))
    }

    @Test
    fun `fails when a word is missing`() {
        assertFalse(backupCheckPasses(phrase, listOf(0, 6, 11), listOf("legal", "", "yellow")))
    }
}
