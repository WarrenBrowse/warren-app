package com.warrenbrowse.vpn.feature.settings.impl

import com.warrenbrowse.vpn.lib.model.ThemePreference
import com.warrenbrowse.vpn.lib.ui.resource.R
import kotlin.test.assertEquals
import org.junit.jupiter.api.Test

class ThemeSettingTest {

    @Test
    fun `the options are offered in the desktop order, following the system first`() {
        assertEquals(
            listOf(ThemePreference.SYSTEM, ThemePreference.DARK, ThemePreference.LIGHT),
            themeOptions,
        )
    }

    @Test
    fun `each option is named by its own string`() {
        assertEquals(R.string.theme_system, ThemePreference.SYSTEM.labelRes())
        assertEquals(R.string.theme_dark, ThemePreference.DARK.labelRes())
        assertEquals(R.string.theme_light, ThemePreference.LIGHT.labelRes())
    }
}
