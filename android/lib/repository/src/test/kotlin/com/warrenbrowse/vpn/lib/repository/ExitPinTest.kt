package com.warrenbrowse.vpn.lib.repository

import kotlin.test.assertFalse
import kotlin.test.assertTrue
import org.junit.jupiter.api.Test

/**
 * The picker-row markers of a pin. Resolving a pin to the exit the engine dials
 * is the Rust rule behind [ExitPinResolver] (`warren-jni/src/exit_pin.rs`), tested
 * there and through the JNI contract in `JniExitPinResolverTest`.
 */
class ExitPinTest {
    @Test
    fun `a country pin marks its country row and nothing deeper`() {
        assertTrue(ExitPin.Country("DE").pinsCountry("de"))
        assertFalse(ExitPin.Country("DE").pinsCountry("FR"))
        assertFalse(ExitPin.City("DE", "Berlin").pinsCountry("DE"))
        assertFalse(ExitPin.Automatic.pinsCountry("DE"))
    }

    @Test
    fun `a city pin marks its city row and nothing deeper`() {
        assertTrue(ExitPin.City("DE", "Berlin").pinsCity("de", "berlin"))
        assertFalse(ExitPin.City("DE", "Berlin").pinsCity("DE", "Frankfurt"))
        assertFalse(ExitPin.Country("DE").pinsCity("DE", "Berlin"))
        assertFalse(ExitPin.Automatic.pinsCity("DE", "Berlin"))
    }
}
