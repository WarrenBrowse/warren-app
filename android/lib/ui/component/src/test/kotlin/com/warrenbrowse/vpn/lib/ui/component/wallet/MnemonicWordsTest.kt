package com.warrenbrowse.vpn.lib.ui.component.wallet

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

class MnemonicWordsTest {
    private val phrase = "abandon ability able about above absent absorb abstract absurd abuse access accident"

    @Test
    fun `a revealed phrase shows its words in order`() {
        assertEquals(phrase.split(' '), mnemonicWordsShown(phrase, revealed = true))
    }

    @Test
    fun `a phrase not yet revealed composes none of its words`() {
        val shown = mnemonicWordsShown(phrase, revealed = false)

        assertEquals(12, shown.size)
        assertTrue(
            shown.none { it in phrase.split(' ') } && shown.toSet().size == 1,
            "the blur is ignored before Android 12, so the words must not be drawn at all",
        )
    }
}
