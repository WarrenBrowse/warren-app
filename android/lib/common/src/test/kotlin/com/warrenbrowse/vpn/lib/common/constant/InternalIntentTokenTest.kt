package com.warrenbrowse.vpn.lib.common.constant

import kotlin.test.assertFalse
import kotlin.test.assertTrue
import org.junit.jupiter.api.Test

class InternalIntentTokenTest {

    @Test
    fun an_intent_built_by_this_process_is_recognised() {
        assertTrue(InternalIntentToken.isGenuine(InternalIntentToken.value))
    }

    /**
     * Another app can send the exported activity the same action, but not the
     * secret: without it, or with a guess, the action is not honoured.
     */
    @Test
    fun an_intent_without_the_secret_is_not() {
        assertFalse(InternalIntentToken.isGenuine(null))
        assertFalse(InternalIntentToken.isGenuine(""))
        assertFalse(InternalIntentToken.isGenuine(InternalIntentToken.value.dropLast(1)))
        assertFalse(InternalIntentToken.isGenuine("0".repeat(InternalIntentToken.value.length)))
    }

    @Test
    fun the_secret_is_long_enough_not_to_be_guessed() {
        assertTrue(InternalIntentToken.value.length >= 64)
    }
}
