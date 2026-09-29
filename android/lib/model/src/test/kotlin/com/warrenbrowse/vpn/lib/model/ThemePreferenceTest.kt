package com.warrenbrowse.vpn.lib.model

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Test

class ThemePreferenceTest {

    @Test
    fun `the stored value of each preference reads back as that preference`() {
        ThemePreference.entries.forEach { preference ->
            assertEquals(preference, ThemePreference.parse(preference.storageValue))
        }
    }

    @Test
    fun `the stored values are the desktop ones, so a shared setting reads alike`() {
        assertEquals(
            listOf("system", "dark", "light"),
            ThemePreference.entries.map { it.storageValue },
        )
    }

    @Test
    fun `a missing or unknown stored value follows the system`() {
        assertEquals(ThemePreference.SYSTEM, ThemePreference.parse(null))
        assertEquals(ThemePreference.SYSTEM, ThemePreference.parse("sepia"))
    }

    @Test
    fun `following the system takes the system theme`() {
        assertEquals(ColorTheme.LIGHT, ThemePreference.SYSTEM.resolve(ColorTheme.LIGHT))
        assertEquals(ColorTheme.DARK, ThemePreference.SYSTEM.resolve(ColorTheme.DARK))
    }

    @Test
    fun `a system that states no theme paints dark`() {
        assertEquals(ColorTheme.DARK, ThemePreference.SYSTEM.resolve(null))
    }

    @Test
    fun `an explicit choice wins over the system`() {
        assertEquals(ColorTheme.DARK, ThemePreference.DARK.resolve(ColorTheme.LIGHT))
        assertEquals(ColorTheme.LIGHT, ThemePreference.LIGHT.resolve(ColorTheme.DARK))
        assertEquals(ColorTheme.LIGHT, ThemePreference.LIGHT.resolve(null))
    }
}
