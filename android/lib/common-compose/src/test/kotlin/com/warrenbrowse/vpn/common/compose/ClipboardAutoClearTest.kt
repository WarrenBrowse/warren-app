package com.warrenbrowse.vpn.common.compose

import kotlin.test.assertEquals
import org.junit.jupiter.api.Test

class ClipboardAutoClearTest {

    @Test
    fun `clears when the clipboard still holds the copied secret`() {
        assertEquals(true, shouldClearClipboard(currentClipText = SECRET, copiedText = SECRET))
    }

    @Test
    fun `keeps the clipboard when another app has replaced the clip`() {
        assertEquals(
            false,
            shouldClearClipboard(currentClipText = "a shopping list", copiedText = SECRET),
        )
    }

    @Test
    fun `keeps the clipboard when the clip cannot be read`() {
        // Reading the primary clip from the background answers null on
        // Android 10+. An unreadable clip is not proof the secret is still
        // there, and clearing blind would drop whatever another app owns.
        assertEquals(false, shouldClearClipboard(currentClipText = null, copiedText = SECRET))
    }

    @Test
    fun `keeps the clipboard when the clip only starts with the copied text`() {
        assertEquals(
            false,
            shouldClearClipboard(currentClipText = "$SECRET and more", copiedText = SECRET),
        )
    }

    private companion object {
        const val SECRET =
            "abandon abandon abandon abandon abandon abandon " +
                "abandon abandon abandon abandon abandon about"
    }
}
