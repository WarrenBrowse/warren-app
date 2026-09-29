package com.warrenbrowse.vpn.app.design

import android.content.res.Configuration
import com.warrenbrowse.vpn.lib.model.ColorTheme
import com.warrenbrowse.vpn.lib.ui.theme.color.WarrenSurfaces
import kotlin.test.assertEquals
import kotlin.test.assertNull
import org.junit.jupiter.api.Test

class SystemColorThemeTest {

    @Test
    fun `a device in night mode states the dark theme`() {
        assertEquals(
            ColorTheme.DARK,
            systemColorTheme(Configuration.UI_MODE_NIGHT_YES or Configuration.UI_MODE_TYPE_NORMAL),
        )
    }

    @Test
    fun `a device out of night mode states the light theme`() {
        assertEquals(
            ColorTheme.LIGHT,
            systemColorTheme(Configuration.UI_MODE_NIGHT_NO or Configuration.UI_MODE_TYPE_NORMAL),
        )
    }

    @Test
    fun `a device that leaves night mode undefined states no theme`() {
        assertNull(
            systemColorTheme(
                Configuration.UI_MODE_NIGHT_UNDEFINED or Configuration.UI_MODE_TYPE_TELEVISION
            )
        )
    }

    @Test
    fun `each resolved theme paints the connect screen with its own surfaces`() {
        assertEquals(WarrenSurfaces.Dark, ColorTheme.DARK.surfaces())
        assertEquals(WarrenSurfaces.Light, ColorTheme.LIGHT.surfaces())
    }
}
