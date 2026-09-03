package com.warrenbrowse.vpn.app.design

import com.warrenbrowse.vpn.core.animation.ENTER_TRANSITION_SLIDE_FACTOR
import com.warrenbrowse.vpn.core.animation.RECEDE_SLIDE_FACTOR
import com.warrenbrowse.vpn.core.animation.TRANSITION_DEFAULT_DURATION_MS
import com.warrenbrowse.vpn.lib.ui.theme.tokens.DesignTokens
import kotlin.test.assertEquals
import org.junit.jupiter.api.Test

/**
 * The screen push against the desktop `transition-hooks.ts` shape (through
 * the generated tokens): full travel for the arriving screen, a third of the
 * width for the receding one. The duration is the one deliberate deviation,
 * pinned here next to the desktop value so neither side moves unnoticed.
 */
class DesignMotionParityTest {

    @Test
    fun `a pushed screen travels the full width, as on desktop`() {
        assertEquals(DesignTokens.Navigation.PushNewFrom, ENTER_TRANSITION_SLIDE_FACTOR)
    }

    @Test
    fun `the screen underneath recedes by the desktop third`() {
        assertEquals(-DesignTokens.Navigation.PushOldTo, RECEDE_SLIDE_FACTOR)
    }

    @Test
    fun `the push runs at the phone ceiling under the desktop duration`() {
        assertEquals(450, DesignTokens.Navigation.Duration)
        assertEquals(350, TRANSITION_DEFAULT_DURATION_MS)
    }
}
